# tiered-memory-engine

AI-agnostic plugin that extends context windows through automatic tiered compression of memory artifacts, with optional post-quantum encryption.

Your AI assistant forgets everything between sessions. The workaround is memory files, conversation logs, and artifact caches. These grow without bound, slow down context loading, and sit unencrypted on disk.

tiered-memory-engine fixes this by applying the same tiered storage architecture that Splunk, Elasticsearch, and cloud datalakes use at scale — but for AI memory artifacts.

## What it does

**Extends context windows** by compressing old memories and building a searchable semantic index. Your assistant loads compact summaries instead of full files, fitting more knowledge into the same token budget.

**Auto-compresses** artifacts across three tiers:

| Tier | Compression | Speed | When |
|------|------------|-------|------|
| Hot | None | Instant | Active session files |
| Warm | zstd level 3 (~3.2x) | ~234 MB/s | 48h old + 24h idle |
| Cold | zstd level 9 (~3.5x) | ~40 MB/s | 14d old + 7d idle |

**Optionally encrypts** with post-quantum cryptography. Every artifact gets its own unique encryption key (envelope encryption), and the key wrapping uses **ML-KEM-768** (NIST FIPS 203) — the strongest publicly standardized post-quantum Key Encapsulation Mechanism available today.

## Context enhancement

Without tiered-memory-engine, your AI assistant loads raw files into its context window. A 128K token window fills fast.

With it:
- **Semantic index** — every artifact is keyword-indexed at registration. Search without decompressing.
- **Progressive recall** — summaries load first (~10-20% of tokens), full content only on demand.
- **Budget management** — tracks token usage, prevents context overflow.
- **Relevance scoring** — matches your current task against all memories, surfaces the most relevant.

```bash
# Get context-optimized memory for your AI session
tiered-memory context --query "security encryption" --budget 128000

# Search memories without decompressing anything
tiered-memory search "TARA threat model"
```

## Post-Quantum Encryption (Optional)

### What is ML-KEM-768?

**ML-KEM-768** (Module-Lattice-Based Key-Encapsulation Mechanism) is the NIST-standardized post-quantum algorithm finalized in August 2024 as **FIPS 203**. It provides Category 3 security (~AES-192 equivalent) against both classical and quantum computers.

age v1.3.0+ uses ML-KEM-768 in **hybrid mode with X25519**, meaning your data is protected by two independent algorithms. Even if one is broken, the other still holds.

This is the same algorithm that OpenSSH 10.0 made the default for key exchange in April 2025.

### This is the most secure option — if you store your secrets safely and always rotate.

Post-quantum encryption is only as strong as your key management:

- **Store private keys in hardware** — macOS Keychain with Touch ID (Secure Enclave on Apple Silicon), YubiKey, or cloud HSMs. Never as plaintext files. The `file:` key source is deliberately blocked in this tool.
- **Rotate keys regularly** — key rotation re-wraps DEK headers in O(metadata), not O(data). No need to re-encrypt terabytes. Do it monthly or on any suspected compromise.
- **Use separate keypairs per tier** — warm and cold tiers get independent keypairs. Compromising one doesn't expose the other.

### How the encryption works

```
ENCRYPTION (uses public key only — no secrets needed):

  config.json (safe to commit, safe to share)
    ├── warm_pubkey: age1abc...   ← encrypts warm-tier artifact DEKs
    └── cold_pubkey: age1xyz...   ← encrypts cold-tier artifact DEKs

  For each artifact:
    1. Generate random 256-bit Data Encryption Key (DEK)
    2. Encrypt DEK with tier's public key via age (ML-KEM-768 + X25519)
    3. Each encryption creates a fresh ephemeral keypair (forward secrecy)
    4. Store encrypted DEK in .envelope.json header alongside artifact


DECRYPTION (recall only — private key retrieved on-demand):

  macOS Keychain / Vault / KMS
    ├── warm private key  ← Touch ID to access on macOS
    └── cold private key  ← Touch ID to access on macOS

  For recall:
    1. Retrieve private key from vault
    2. Decrypt the artifact's DEK
    3. Decrypt artifact data
    4. Zero private key from memory
```

### Private key sources (ranked by security)

| Source | Format | Best For |
|--------|--------|----------|
| macOS Keychain + Touch ID | `keychain:tiered-memory:warm-key` | Local dev (hardware-bound on M-series) |
| HashiCorp Vault | `command:vault kv get -field=key secret/tm/warm` | Teams, servers |
| Cloud KMS | `command:aws secretsmanager get-secret-value ...` | Cloud infrastructure |
| Environment variable | `env:TM_WARM_PRIVKEY` | CI/CD pipelines only |

**Not supported:** `file:` — private keys must never exist as plaintext files on disk.

See [docs/KEY-STORAGE-GUIDE.md](docs/KEY-STORAGE-GUIDE.md) for detailed setup instructions and risk profiles for each vault option.

## Quick start

```bash
# Install
pip install -e .

# Initialize config (auto-detects Claude, ChatGPT, Cursor, Copilot artifacts)
tiered-memory init

# Scan for artifacts
tiered-memory scan

# Preview what would be compressed (dry run)
tiered-memory run --dry-run

# Run tiering
tiered-memory run

# Check status
tiered-memory status

# Get context for AI injection
tiered-memory context --query "your current task"

# Search memories
tiered-memory search "keyword"

# Recall a cold artifact back to hot
tiered-memory recall /path/to/original/file
```

### Enable encryption

```bash
# Guided setup
tiered-memory encrypt-setup

# Or one-command setup with Touch ID (macOS):
python3 -c "
from src.envelope import EnvelopeEncryptor
pub, src = EnvelopeEncryptor.setup_tier_with_keychain('warm')
print(f'Public key: {pub}')
print(f'Source: {src}')
# Private key went straight from age-keygen to Keychain.
# It never existed as a file.
"
```

## AI-agnostic

Works with any AI assistant that uses file-based memory:

| Assistant | Auto-detected Artifacts |
|-----------|------------------------|
| Claude Code | `~/.claude/subagents/*.jsonl`, project memory, todos |
| ChatGPT | Desktop app cache |
| Cursor | Conversation logs |
| GitHub Copilot | Configuration cache |
| Custom | Add any path to `config.json` |

## Architecture

Inspired by production datalake tiering:

| Component | Inspired By | What It Does |
|-----------|-------------|--------------|
| Tier transitions | Splunk SmartStore bucket lifecycle | Age + idle time trigger compression |
| Compression codec | Splunk 7.2+ / ELK 8.17+ / Iceberg 1.4+ default | zstd (fastest ratio/speed) |
| ILM phases | Elasticsearch Index Lifecycle Management | Hot → Warm → Cold with configurable thresholds |
| Searchable index | Elasticsearch inverted index | Keyword search without decompressing |
| Progressive recall | ELK frozen-tier partially mounted snapshots | Summary first, full content on demand |
| Envelope encryption | AWS KMS / Google Tink | Per-artifact DEK, asymmetric key wrap |
| Context budget | Splunk license metering | Track and limit token usage |

## Security

- Per-artifact encryption keys (compromise one artifact, others stay safe)
- Per-tier keypairs (compromise warm, cold stays safe)
- Forward secrecy (age uses ephemeral keys per encryption)
- SHA-256 integrity verification on compress and recall
- Symlink protection (prevents cross-directory reads)
- Path containment (scan targets confined to home directory)
- Sensitive directory blocklist (~/.ssh, ~/.gnupg, ~/.aws blocked)
- Temp files use unpredictable names with 0600 permissions
- Config and metadata files written atomically with 0600/0700
- Registry field filtering (unknown fields dropped, prevents injection)
- No `shell=True` subprocess calls anywhere
- Private key file source deliberately disabled

Red-team reviewed by 3 independent security personas (offensive, crypto, supply chain).

## Requirements

- Python 3.10+
- `zstandard` >= 0.19.0 (installed automatically)
- `age` >= 1.3.0 (optional, for PQ encryption) — `brew install age`

## Tests

```bash
pip install -e ".[dev]"
pytest tests/ -v
```

## License

MIT
