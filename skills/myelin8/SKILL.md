---
name: myelin8
description: Extends AI context windows by modeling how the brain stores memory — short-term stays fast, long-term compresses deep. Four storage tiers (hot/warm/cold/frozen) with 4-50x compression ratios, semantic search across all tiers without decompressing, and optional post-quantum encryption (ML-KEM-768) to protect your AI's memory at rest. Use when the user mentions memory, context window, compression, searching old sessions, recalling past conversations, encrypting AI data, or managing storage.
allowed-tools: Read, Bash(myelin8 *), Bash(python3 -m src.cli *), Grep, Glob
user-invocable: true
argument-hint: "[command or example name]"
---

# Myelin8

You are managing an AI memory tiering system. When invoked, help the user with memory compression, search, recall, encryption, and context optimization.

## Examples

When the user runs `/myelin8 usecases` or `/myelin8 examples`, show this list and ask which they'd like to do:

When the user passes arguments like `/myelin8 usecases`, treat "usecases" as the command and show the full examples list below.

### 1. First-time setup
```bash
# Auto-detect AI assistants, choose tiers, configure thresholds
myelin8 init

# See what's on your system before changing anything
myelin8 scan
myelin8 run --dry-run
```
**What it does:** Discovers all AI session files (Claude, ChatGPT, Cursor, etc.), shows how many artifacts you have, how much disk they use, and what would move to each tier. No files are modified.

### 2. Compress old sessions
```bash
# Move idle artifacts to warm/cold/frozen tiers
myelin8 run

# Check the results
myelin8 status
```
**What it does:** Files older than 1 week get minified + compressed (warm, 4-5x). Files older than 1 month get boilerplate-stripped + dictionary-compressed (cold, 8-12x). Files older than 3 months get converted to columnar Parquet (frozen, 20-50x). Recent files stay untouched.

### 3. Search past conversations
```bash
# Find something across all tiers without decompressing
myelin8 search "authentication refactor"
myelin8 search "that bug we fixed in January"
myelin8 search "TARA threat model"
```
**What it does:** Searches the semantic index (132K+ keywords). Returns matching artifacts with tier, age, summary, and relevance score. No decompression needed — the index is always in memory.

### 4. Load context for this session
```bash
# Get a token-budget-optimized memory block
myelin8 context --query "auth patterns we discussed"
myelin8 context --query "security architecture decisions"
```
**What it does:** Builds a context block from your most relevant memories. Hot summaries load first, then query-relevant warm/cold/frozen matches. Fits within your token budget. Output is plain text you can paste into any AI prompt.

### 5. Recall a frozen artifact
```bash
# Bring a deep-archived session back to hot tier
myelin8 recall ~/.claude/projects/.../session-2025-06-15.jsonl
```
**What it does:** Decompresses (and decrypts if encrypted) a cold/frozen artifact back to hot tier. Cold takes ~500ms. Frozen takes ~5 seconds. The file is fully accessible again. Run `myelin8 run` later to re-tier it when idle.

### 6. Enable post-quantum encryption
```bash
# Build the crypto sidecar (handles keys — Python never sees them)
cd myelin8/sidecar && cargo build --release

# Generate per-tier keypairs stored in Keychain (Touch ID)
myelin8 encrypt-setup
```
**What it does:** Sets up ML-KEM-768 hybrid encryption via a Rust sidecar. No external tools needed — everything is built in. Private keys go directly to macOS Keychain — they never exist as files, never enter Python, never appear in terminal output. Every tier gets its own keypair. Every artifact gets its own unique encryption key.

### 7. Set up 2-tier mode (simple)
```bash
# During init, choose 2-tier mode
myelin8 init
# When prompted: "How many storage tiers?" → choose 2
```
**What it does:** Simplified mode with just Hot (recent) and Cold (old, compressed). Fewer moving parts. Good for most users who want compression without complexity.

### 8. Set up 4-tier mode (maximum compression)
```bash
# During init, choose 4-tier mode
myelin8 init
# When prompted: "How many storage tiers?" → choose 4
```
**What it does:** Full pipeline — Hot / Warm (4-5x) / Cold (8-12x) / Frozen (20-50x). Each tier applies different compression strategies. Best disk savings. Recommended for heavy AI users.

### 9. Customize tier thresholds
```bash
# Edit the config directly
cat ~/.myelin8/config.json
```
**Example thresholds:**
```json
{
  "tier_policy": {
    "hot_to_warm_age_hours": 24,
    "hot_to_warm_idle_hours": 12,
    "warm_to_cold_age_hours": 168,
    "warm_to_cold_idle_hours": 72,
    "cold_to_frozen_age_hours": 720,
    "cold_to_frozen_idle_hours": 336
  }
}
```
**What it does:** Moves artifacts faster or slower than defaults (1 week to warm, 1 month to cold, 3 months to frozen). Set any threshold to match your workflow.

### 10. Enable most secure mode (encrypt everything including hot)
```bash
# During init, enable encryption, then choose "most secure"
myelin8 init
# Enable PQ encryption? → yes
# Enable most secure mode (encrypt all tiers including hot)? → yes
```
**What it does:** Encrypts every file at rest, including active session files in hot tier. Requires Touch ID on every file read. Maximum security — no plaintext on disk at any tier. Slower reads but nothing is ever unprotected.

### 11. Check what you're wasting
```bash
myelin8 status
```
```
Total artifacts:   4,564
  Hot:             4,237
  Warm:            41
  Cold:            286
  Frozen:          0
Total original:    2.6 GB
Total compressed:  222 MB
Overall ratio:     11.62x
Space saved:       1.6 GB
Indexed:           132,081 keywords
```

### 12. Verify data integrity
```bash
# Check SHA-256 hashes of all tracked artifacts
myelin8 verify
```
**What it does:** Computes the current hash of every tracked artifact and compares against the stored hash. Reports passed, failed, and skipped (no hash stored). Failed = file was modified or corrupted since registration.

### 13. Rebuild the search index
```bash
# If the index is corrupted or you want a fresh start
myelin8 reindex
```
**What it does:** Deletes the semantic index and re-scans all artifacts. Re-extracts keywords and summaries. Use after moving files, changing scan targets, or recovering from corruption.

## Available Commands

| Command | What |
|---------|------|
| `myelin8 init` | Guided setup (2-tier or 4-tier, encryption choice) |
| `myelin8 scan` | Discover artifacts |
| `myelin8 run` | Execute tier transitions |
| `myelin8 run --dry-run` | Preview without changes |
| `myelin8 status` | Tier distribution and stats |
| `myelin8 search <query>` | Search all tiers |
| `myelin8 context --query <q>` | Budget-optimized context block |
| `myelin8 recall <path>` | Decompress to hot |
| `myelin8 reindex` | Rebuild semantic index |
| `myelin8 verify` | SHA-256 integrity check |
| `myelin8 encrypt-setup` | Configure encryption |
