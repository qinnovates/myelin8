# Key Storage Guide

How Myelin8 manages encryption keys across platforms.

---

## How keys work in Myelin8

Myelin8 uses ML-KEM-768 (NIST FIPS 203) + X25519 hybrid key encapsulation with AES-256-GCM data encryption. All private key operations happen inside a compiled Rust sidecar (`myelin8-vault`). Private keys never enter Python, never touch disk as files, and never appear in terminal output or process arguments.

The sidecar retrieves keys from your configured source, holds them in mlocked memory, performs in-process crypto, and zeros them immediately after.

---

## Setup

```bash
# Generate per-tier ML-KEM-768 keypairs, store directly in macOS Keychain
myelin8 encrypt-setup
```

Or via the sidecar directly:
```bash
echo "KEYGEN warm" | myelin8-vault
echo "KEYGEN cold" | myelin8-vault
echo "KEYGEN frozen" | myelin8-vault
```

Private keys go straight into the OS credential vault. They never exist as files.

---

## Supported key sources

Configure in `~/.myelin8/config.json`:

### macOS Keychain
```json
"warm_private_source": "keychain:myelin8:warm-key"
```
Sidecar reads from Keychain via Security.framework. Protected by login password.

### External command (Vault, KMS, custom)
```json
"warm_private_source": "command:vault kv get -field=key secret/myelin8/warm"
```
Sidecar calls the command, captures key from stdout, uses it, zeros it. The command must output only the key. Only allowlisted executables permitted (vault, op, aws, gcloud, az, security, gpg, sops, pass).

### Environment variable (CI/CD only)
```json
"warm_private_source": "env:MYELIN8_WARM_KEY"
```
For ephemeral CI/CD runners only. Not for persistent machines.

### File-based keys
**Blocked.** The `file:` source raises an error. Keys must not exist as plaintext files on disk.

---

## Per-tier key isolation

Each tier gets its own independent ML-KEM-768 keypair:

| Tier | Keychain entry | Purpose |
|------|---------------|---------|
| warm | `myelin8:warm-key` | Recent sessions (1-4 weeks old) |
| cold | `myelin8:cold-key` | Older sessions (1-3 months) |
| frozen | `myelin8:frozen-key` | Archival (3+ months) |

Compromising one tier's key does not expose the others. Key rotation re-wraps DEK headers in O(metadata), not O(data).

---

## Key lifecycle

| Step | What happens |
|------|-------------|
| **1. Generation** | Sidecar generates ML-KEM-768 + X25519 hybrid keypair in-process |
| **2. Storage** | Private key stored in OS credential vault (Keychain on macOS, libsecret on Linux). Public key returned to Python for config |
| **3. Encryption** | Public key only — sidecar encapsulates a shared secret, derives AES-256-GCM key via HKDF, encrypts data. No private key involved |
| **4. Decryption** | Sidecar retrieves private key from credential vault into mlocked memory, decapsulates shared secret, decrypts, zeros key |
| **5. Never** | Private key touches disk, enters Python, appears in process args, passes through env vars, gets logged, hits swap (mlockall), appears in core dumps (RLIMIT_CORE=0) |

---

## Algorithms

| Component | Algorithm | Standard |
|-----------|-----------|----------|
| Key encapsulation | ML-KEM-768 + X25519 hybrid | NIST FIPS 203 |
| Data encryption | AES-256-GCM | NIST FIPS 197 + SP 800-38D |
| Key derivation | HKDF-SHA256 | NIST SP 800-56C |
| Merkle tree | SHA3-256 | NIST FIPS 202 |
| Root seal | HMAC-SHA3-256 | NIST FIPS 198-1 |
| Memory protection | mlockall + RLIMIT_CORE=0 | POSIX |
| Key zeroing | Zeroizing\<T\> (deterministic) | NIST SP 800-57 Part 1 §8.3 |
