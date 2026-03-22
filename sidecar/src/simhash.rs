//! SimHash — semantic fingerprinting using SHA3-256 as the hash function.
//!
//! Converts text into a fixed-size binary fingerprint where texts with
//! overlapping words produce similar fingerprints. No model file. No ML
//! runtime. No external dependencies. SHA3-256 IS the embedding model.
//!
//! Algorithm:
//!   1. Tokenize text into words (lowercase, strip punctuation)
//!   2. For each word: SHA3-256(word) → 256 bits → treat as +1/-1 vector
//!   3. Weight by word frequency (TF) and inverse document frequency (IDF)
//!   4. Sum all weighted word vectors
//!   5. Take sign of each dimension → 256-bit fingerprint
//!
//! Similarity: Hamming distance between fingerprints.
//! Close Hamming = similar word overlap. O(1) comparison via XOR + popcount.
//!
//! This runs inside the sidecar alongside the Merkle tree and keyword index.
//! Zero additional dependencies — reuses SHA3-256 from merkle.rs.

use sha3::{Sha3_256, Digest};
use std::collections::HashMap;

/// 256-bit SimHash fingerprint (32 bytes)
pub type Fingerprint = [u8; 32];

/// Stop words to exclude from fingerprinting
const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
    "have", "has", "had", "do", "does", "did", "will", "would", "could",
    "should", "may", "might", "can", "to", "of", "in", "for", "on",
    "with", "at", "by", "from", "as", "into", "through", "during",
    "before", "after", "above", "below", "between", "then", "once",
    "here", "there", "when", "where", "why", "how", "all", "each",
    "every", "both", "few", "more", "most", "other", "some", "such",
    "no", "not", "only", "same", "so", "than", "too", "very", "just",
    "now", "also", "but", "and", "or", "if", "this", "that", "these",
    "those", "it", "its", "they", "them", "their", "we", "our", "you",
    "your", "me", "my", "he", "she", "his", "her", "what", "which",
    "who", "let", "use", "make", "like", "get", "got", "one", "two",
    "new", "about", "up", "out", "much", "right", "still", "thing",
];

/// Tokenize text into lowercase words, filtering stop words and short tokens.
fn tokenize(text: &str) -> Vec<String> {
    let stop: std::collections::HashSet<&str> = STOP_WORDS.iter().copied().collect();

    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.len() >= 3 && !stop.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Generate character n-gram shingles for short-text robustness.
/// "merkle tree" → ["mer", "erk", "rkl", "kle", "le ", "e t", " tr", "tre", "ree"]
/// More robust than word-level on short documents (<50 tokens).
fn shingle(text: &str, n: usize) -> Vec<String> {
    let lower = text.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() < n {
        return vec![lower];
    }
    chars.windows(n)
        .map(|w| w.iter().collect::<String>())
        .collect()
}

/// Minimum similarity threshold for SimHash to contribute to RRF.
/// Below this, the SimHash signal is noise, not signal.
pub const MIN_SIMHASH_SIMILARITY: f64 = 0.55;

/// Hash a word to a 256-bit vector using SHA3-256.
fn word_hash(word: &str) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(word.as_bytes());
    hasher.finalize().into()
}

/// Compute SimHash fingerprint for a text string.
///
/// Each word contributes its SHA3-256 hash weighted by frequency.
/// The final fingerprint is the sign of each bit position across all words.
pub fn compute(text: &str) -> Fingerprint {
    let tokens = tokenize(text);
    if tokens.is_empty() {
        return [0u8; 32];
    }

    // Count word frequencies (TF)
    let mut freq: HashMap<String, f64> = HashMap::new();
    for token in &tokens {
        *freq.entry(token.clone()).or_insert(0.0) += 1.0;
    }

    // Accumulator: 256 dimensions, signed
    let mut acc = [0.0f64; 256];

    for (word, count) in &freq {
        let hash = word_hash(word);
        let weight = *count; // TF weighting

        // Each bit of the hash contributes +weight or -weight
        for byte_idx in 0..32 {
            for bit_idx in 0..8 {
                let dim = byte_idx * 8 + bit_idx;
                if (hash[byte_idx] >> (7 - bit_idx)) & 1 == 1 {
                    acc[dim] += weight;
                } else {
                    acc[dim] -= weight;
                }
            }
        }
    }

    // Take sign → binary fingerprint
    let mut fingerprint = [0u8; 32];
    for byte_idx in 0..32 {
        let mut byte_val = 0u8;
        for bit_idx in 0..8 {
            let dim = byte_idx * 8 + bit_idx;
            if acc[dim] >= 0.0 {
                byte_val |= 1 << (7 - bit_idx);
            }
        }
        fingerprint[byte_idx] = byte_val;
    }

    fingerprint
}

/// Compute SimHash with IDF weighting + character n-gram shingling.
/// Combines word-level TF-IDF (good for long text) with 4-gram shingles
/// (robust for short text). Both contribute to the final fingerprint.
pub fn compute_weighted(text: &str, idf: &HashMap<String, f64>) -> Fingerprint {
    let tokens = tokenize(text);
    let shingles = shingle(text, 4);

    if tokens.is_empty() && shingles.is_empty() {
        return [0u8; 32];
    }

    let mut acc = [0.0f64; 256];

    // Word-level features (weighted by TF-IDF, 70% of signal)
    let mut freq: HashMap<String, f64> = HashMap::new();
    for token in &tokens {
        *freq.entry(token.clone()).or_insert(0.0) += 1.0;
    }

    for (word, tf) in &freq {
        let hash = word_hash(word);
        let word_idf = idf.get(word).copied().unwrap_or(1.0);
        let weight = tf * word_idf * 0.7;

        for byte_idx in 0..32 {
            for bit_idx in 0..8 {
                let dim = byte_idx * 8 + bit_idx;
                if (hash[byte_idx] >> (7 - bit_idx)) & 1 == 1 {
                    acc[dim] += weight;
                } else {
                    acc[dim] -= weight;
                }
            }
        }
    }

    // Character n-gram features (30% of signal, robust on short text)
    let shingle_weight = 0.3 / shingles.len().max(1) as f64;
    for sg in &shingles {
        let hash = word_hash(sg);
        for byte_idx in 0..32 {
            for bit_idx in 0..8 {
                let dim = byte_idx * 8 + bit_idx;
                if (hash[byte_idx] >> (7 - bit_idx)) & 1 == 1 {
                    acc[dim] += shingle_weight;
                } else {
                    acc[dim] -= shingle_weight;
                }
            }
        }
    }

    let mut fingerprint = [0u8; 32];
    for byte_idx in 0..32 {
        let mut byte_val = 0u8;
        for bit_idx in 0..8 {
            let dim = byte_idx * 8 + bit_idx;
            if acc[dim] >= 0.0 {
                byte_val |= 1 << (7 - bit_idx);
            }
        }
        fingerprint[byte_idx] = byte_val;
    }

    fingerprint
}

/// Hamming similarity between two fingerprints (0.0 = opposite, 1.0 = identical).
/// Uses XOR + popcount — O(1), constant-time friendly.
pub fn similarity(a: &Fingerprint, b: &Fingerprint) -> f64 {
    let mut matching_bits: u32 = 0;
    for i in 0..32 {
        // XOR gives 1 for differing bits, count zeros = matching bits
        matching_bits += (!(a[i] ^ b[i])).count_ones();
    }
    matching_bits as f64 / 256.0
}

/// Hamming distance (number of differing bits, 0-256).
pub fn distance(a: &Fingerprint, b: &Fingerprint) -> u32 {
    let mut diff: u32 = 0;
    for i in 0..32 {
        diff += (a[i] ^ b[i]).count_ones();
    }
    diff
}

/// SimHash index — stores fingerprints for all artifacts and enables fast similarity search.
pub struct SimHashIndex {
    /// artifact hash → fingerprint
    fingerprints: Vec<(String, Fingerprint)>,
    /// IDF scores (built from corpus)
    idf: HashMap<String, f64>,
    /// Total documents (for IDF computation)
    doc_count: usize,
    /// Document frequency per word
    df: HashMap<String, usize>,
    /// Pending texts awaiting finalization (bulk load optimization)
    pending_texts: Vec<(String, String)>,
}

impl SimHashIndex {
    pub fn new() -> Self {
        Self {
            fingerprints: Vec::new(),
            idf: HashMap::new(),
            doc_count: 0,
            df: HashMap::new(),
            pending_texts: Vec::new(),
        }
    }

    /// Add a document to the index. Call `finalize()` after bulk adds.
    pub fn add(&mut self, artifact_hash: &str, text: &str) {
        // Update document frequency
        let tokens = tokenize(text);
        let unique_words: std::collections::HashSet<&String> = tokens.iter().collect();
        for word in &unique_words {
            *self.df.entry((*word).clone()).or_insert(0) += 1;
        }
        self.doc_count += 1;

        // Store raw text for deferred fingerprint computation
        self.pending_texts.push((artifact_hash.to_string(), text.to_string()));
    }

    /// Finalize the index — compute IDF once, then generate all fingerprints.
    /// Call after bulk loading. O(V + N*T) instead of O(N*V).
    pub fn finalize(&mut self) {
        self.recompute_idf();

        // Compute fingerprints for all pending documents
        for (hash, text) in self.pending_texts.drain(..) {
            let fp = compute_weighted(&text, &self.idf);
            self.fingerprints.push((hash, fp));
        }
    }

    /// Search for similar documents. Returns (artifact_hash, similarity) sorted by similarity desc.
    /// Auto-finalizes if there are pending documents.
    pub fn search(&mut self, query: &str, top_k: usize) -> Vec<(String, f64)> {
        if !self.pending_texts.is_empty() {
            self.finalize();
        }
        let query_fp = compute_weighted(query, &self.idf);

        let mut results: Vec<(String, f64)> = self.fingerprints.iter()
            .map(|(hash, fp)| (hash.clone(), similarity(&query_fp, fp)))
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    /// Get fingerprint count.
    pub fn len(&self) -> usize {
        self.fingerprints.len()
    }

    /// Reset the index.
    pub fn reset(&mut self) {
        self.fingerprints.clear();
        self.idf.clear();
        self.doc_count = 0;
        self.df.clear();
        self.pending_texts.clear();
    }

    fn recompute_idf(&mut self) {
        self.idf.clear();
        let n = self.doc_count as f64;
        for (word, df) in &self.df {
            // Standard IDF: log(N / df) + 1 (smoothed)
            self.idf.insert(word.clone(), (n / (*df as f64)).ln() + 1.0);
        }
    }
}

/// Hex encode a fingerprint.
pub fn hex_encode(fp: &Fingerprint) -> String {
    fp.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Hex decode a fingerprint.
pub fn hex_decode(hex: &str) -> Result<Fingerprint, String> {
    if hex.len() != 64 {
        return Err("Fingerprint must be 64 hex chars".into());
    }
    let bytes: Result<Vec<u8>, _> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| "Invalid hex".to_string()))
        .collect();
    let bytes = bytes?;
    let mut fp = [0u8; 32];
    fp.copy_from_slice(&bytes);
    Ok(fp)
}
