//! Merkle tree — PQC-hardened integrity verification.
//!
//! Hash algorithm: SHA3-256 (NIST FIPS 202)
//!   - 128-bit collision resistance post-quantum (vs SHA-256's ~64-bit)
//!   - 256-bit preimage resistance post-quantum
//!   - Sponge construction (independent of Merkle-Damgård, no length extension)
//!
//! Domain separation:
//!   Leaf:     SHA3-256(0x00 || data)
//!   Internal: SHA3-256(0x01 || left || right)
//!   Prevents second-preimage attacks (RFC 6962 compliant).
//!
//! Root sealing: HMAC-SHA3-256(root, pqc_derived_key)
//!   - Key derived via HKDF from ML-KEM shared secret
//!   - Proves the root was computed by a process with access to PQC key material
//!   - Constant-time comparison on verify
//!
//! All operations run inside the sidecar process (mlocked, zero core dumps).

use sha3::{Sha3_256, Digest};
use hmac::{Hmac, Mac};

type HmacSha3 = Hmac<Sha3_256>;

const LEAF_PREFIX: u8 = 0x00;
const NODE_PREFIX: u8 = 0x01;

fn hash_leaf(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update([LEAF_PREFIX]);
    hasher.update(data);
    hasher.finalize().into()
}

fn hash_node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update([NODE_PREFIX]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

/// Constant-time comparison (prevents timing side channels)
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// PQC-hardened Merkle tree with SHA3-256 and HMAC root sealing.
pub struct MerkleTree {
    leaves: Vec<[u8; 32]>,
    layers: Vec<Vec<[u8; 32]>>,
    dirty: bool,
    /// HMAC seal of the root (set by seal_root)
    root_seal: Option<[u8; 32]>,
}

impl MerkleTree {
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            layers: Vec::new(),
            dirty: true,
            root_seal: None,
        }
    }

    /// Add raw data as a leaf. Returns leaf index.
    pub fn add_data(&mut self, data: &[u8]) -> usize {
        let h = hash_leaf(data);
        self.leaves.push(h);
        self.dirty = true;
        self.root_seal = None; // Seal invalidated
        self.leaves.len() - 1
    }

    /// Add a pre-computed SHA3-256 hash as a leaf (with domain separation).
    pub fn add_hash(&mut self, hash: &[u8; 32]) -> usize {
        let h = hash_leaf(hash);
        self.leaves.push(h);
        self.dirty = true;
        self.root_seal = None;
        self.leaves.len() - 1
    }

    /// Add a hex-encoded hash (64 chars = 32 bytes).
    pub fn add_hex(&mut self, hex: &str) -> Result<usize, String> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 32 {
            return Err("Expected 64-char hex (32 bytes)".into());
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(self.add_hash(&arr))
    }

    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    /// Compute and return the root hash (SHA3-256).
    pub fn root(&mut self) -> Option<[u8; 32]> {
        if self.leaves.is_empty() {
            return None;
        }
        self.rebuild();
        self.layers.last().and_then(|l| l.first().copied())
    }

    pub fn root_hex(&mut self) -> Option<String> {
        self.root().map(|r| hex_encode(&r))
    }

    /// Seal the root with HMAC-SHA3-256 using a PQC-derived key.
    /// The key should come from HKDF(ML-KEM shared secret).
    pub fn seal_root(&mut self, key: &[u8]) -> Result<String, String> {
        let root = self.root().ok_or("Empty tree")?;
        let mut mac = HmacSha3::new_from_slice(key)
            .map_err(|_| "Invalid HMAC key")?;
        mac.update(&root);
        let seal: [u8; 32] = mac.finalize().into_bytes().into();
        self.root_seal = Some(seal);
        Ok(hex_encode(&seal))
    }

    /// Verify a root seal. Constant-time.
    pub fn verify_seal(&mut self, key: &[u8], seal_hex: &str) -> Result<bool, String> {
        let root = self.root().ok_or("Empty tree")?;
        let expected = hex_decode(seal_hex)?;
        if expected.len() != 32 {
            return Err("Seal must be 64 hex chars".into());
        }

        let mut mac = HmacSha3::new_from_slice(key)
            .map_err(|_| "Invalid HMAC key")?;
        mac.update(&root);
        let computed: [u8; 32] = mac.finalize().into_bytes().into();

        Ok(ct_eq(&computed, &expected))
    }

    /// Generate a Merkle proof for the leaf at `index`.
    pub fn proof(&mut self, index: usize) -> Result<MerkleProof, String> {
        if index >= self.leaves.len() {
            return Err(format!("Index {} out of range (0-{})", index, self.leaves.len() - 1));
        }
        self.rebuild();

        let mut siblings = Vec::new();
        let mut directions = Vec::new();
        let mut idx = index;

        for level in 0..self.layers.len() - 1 {
            let layer = &self.layers[level];
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            let dir = if idx % 2 == 0 { "right" } else { "left" };

            if sibling_idx < layer.len() {
                siblings.push(layer[sibling_idx]);
            } else {
                siblings.push(layer[idx]);
            }
            directions.push(dir.to_string());
            idx /= 2;
        }

        let root = self.root().ok_or("Empty tree")?;

        Ok(MerkleProof {
            leaf_hash: self.leaves[index],
            leaf_index: index,
            siblings,
            directions,
            root,
        })
    }

    /// Verify a proof. Constant-time comparison.
    pub fn verify(proof: &MerkleProof) -> bool {
        let mut current = proof.leaf_hash;

        for (sibling, direction) in proof.siblings.iter().zip(proof.directions.iter()) {
            if direction == "right" {
                current = hash_node(&current, sibling);
            } else {
                current = hash_node(sibling, &current);
            }
        }

        ct_eq(&current, &proof.root)
    }

    fn rebuild(&mut self) {
        if !self.dirty || self.leaves.is_empty() {
            return;
        }

        let n = self.leaves.len();
        let target = n.next_power_of_two().max(2);
        let mut padded = self.leaves.clone();
        padded.resize(target, [0u8; 32]);

        self.layers = vec![padded.clone()];
        let mut current = padded;

        while current.len() > 1 {
            let mut next = Vec::with_capacity(current.len() / 2);
            for pair in current.chunks(2) {
                let left = &pair[0];
                let right = if pair.len() > 1 { &pair[1] } else { left };
                next.push(hash_node(left, right));
            }
            self.layers.push(next.clone());
            current = next;
        }

        self.dirty = false;
    }
}

pub struct MerkleProof {
    pub leaf_hash: [u8; 32],
    pub leaf_index: usize,
    pub siblings: Vec<[u8; 32]>,
    pub directions: Vec<String>,
    pub root: [u8; 32],
}

impl MerkleProof {
    pub fn to_response(&self) -> String {
        let siblings_hex: Vec<String> = self.siblings.iter().map(|s| hex_encode(s)).collect();
        format!(
            "OK {} {} {} {} {}",
            hex_encode(&self.leaf_hash),
            self.leaf_index,
            siblings_hex.join(","),
            self.directions.join(","),
            hex_encode(&self.root),
        )
    }
}

// ── Hex utilities ──

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("Odd hex length".into());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| "Invalid hex".into())
        })
        .collect()
}
