# Myelin8 Engineering Proposal

**Author:** Kevin Qi
**Date:** 2026-03-22
**Status:** Active development
**Binary:** 11MB Rust, 2,928 LOC, 18 unit tests passing

---

## Current State Assessment

### What exists and works

The foundation is solid. tantivy indexing is real, search is real, integrity verification is real. The system ingests 34 artifacts from two sources, indexes all tokens with BM25, and returns ranked results in <3ms. SHA-256 integrity passes on every artifact. The MCP server compiles and exposes four tools over stdio JSON-RPC.

### What exists but is untested in production

Parquet tier compaction is scaffolded but has never run against real aged data. The MCP server has never been connected to a live Claude Code session. Supersession detection works in unit tests but hasn't been validated against real decision chains. Semantic KV expansion extracts fields but search quality with synonym expansion hasn't been measured against paraphrased queries at scale.

### What doesn't exist yet

- WAL (crash recovery during compaction)
- File locking (concurrent access safety)
- State tracking (which source files have been seen, mtime-based change detection)
- Query expansion at search time (synonym map is built, not wired to search path)
- Significance from user pins (pin command is scaffolded, doesn't persist)
- Parquet rollup (weekly → monthly → quarterly consolidation)
- Claude Code integration (CLAUDE.md instruction, SessionStart hook)
- Progress output on `myelin8 run` (indicatif is in Cargo.toml, not used)
- MCP server testing against real MCP client
- Binary distribution (brew, GitHub releases)

---

## Architecture: What Gets Built

### The three systems

Myelin8 is three systems that share one index:

```
INGEST SYSTEM          SEARCH SYSTEM          TIER SYSTEM
(write path)           (read path)            (maintenance)

myelin8 add            myelin8 search         myelin8 run (compaction)
myelin8 run (ingest)   myelin8 recall         timer/cron
MCP memory_ingest      MCP memory_search
                       MCP memory_recall

     │                      │                      │
     └──────────┬───────────┘                      │
                │                                  │
         ┌──────▼──────┐                    ┌──────▼──────┐
         │   tantivy   │                    │   Parquet   │
         │   (index)   │◄───────────────────│   (store)   │
         └─────────────┘                    └─────────────┘
```

These three systems must never block each other. Search must work while ingest is running. Compaction must not corrupt the index while a search is reading it. tantivy's architecture handles this natively — readers and writers operate on separate segment snapshots.

### File locking contract

```
tantivy writer lock:   acquired by myelin8 run (ingest + compaction)
                       one writer at a time (tantivy enforces this)

tantivy reader:        acquired by myelin8 search, MCP server
                       unlimited concurrent readers
                       readers see a consistent snapshot
                       new segments become visible after writer commits

Parquet files:         immutable once written
                       write to .tmp/, atomic rename to store/
                       readers open by path — if file disappears mid-read,
                       fall back to tantivy stored summary

hot/ files:            one JSON per artifact
                       flock() on write, no lock on read
                       move to .recycled/ on compaction (not delete)
```

### State tracking

The system needs to know which files it has already seen. Without this, every `myelin8 run` re-ingests everything.

```
state.json:
{
  "seen": {
    "/path/to/file.md": {
      "mtime": 1711123456,
      "content_hash": "a3f8c2..."
    }
  }
}
```

On each run:
1. Scan registered sources
2. For each file: check mtime against state.json
3. If mtime unchanged → skip
4. If mtime changed → re-read, recompute hash
5. If hash unchanged → skip (mtime changed but content didn't)
6. If hash changed → re-ingest, update index

This is the same approach as `make` — check timestamps first (fast), fall back to content comparison only when needed.

---

## The Ingest Pipeline (what happens when content enters the system)

```
Source file detected (new or changed)
│
├─ 1. Read content
│     Plaintext only. Skip binary files.
│     Decompress .gz in memory if needed.
│
├─ 2. Compute SHA-256 content hash
│     This is the artifact's permanent identity.
│     Never recomputed. Travels through every tier.
│
├─ 3. Dedup check
│     Compare hash against state.json seen set.
│     If hash exists → skip (already indexed).
│     SimHash check: if Hamming distance < threshold
│     against existing artifacts → flag as near-duplicate.
│
├─ 4. Extract metadata
│     │
│     ├─ Keywords: ALL unique tokens (tantivy indexes everything)
│     ├─ Summary: template-based (first heading + first paragraph)
│     ├─ Semantic KV:
│     │   ├─ technology: matched against 120+ known terms
│     │   ├─ category: derived from technology matches
│     │   ├─ action: matched against 70+ action verbs
│     │   ├─ domain: matched against 80+ domain signals
│     │   ├─ polarity: positive/negative/neutral
│     │   └─ entities: PascalCase + ALLCAPS tokens
│     ├─ Significance: heuristic score (0.0–1.0)
│     └─ Timestamps: created_date (from file mtime), last_accessed (now)
│
├─ 5. Supersession check
│     Extract topic signature (technology + domain terms).
│     Compare against all existing signatures.
│     If Jaccard similarity > 0.7 AND same type AND newer date:
│       → Mark old artifact as superseded
│       → Link new artifact to old via supersedes field
│
├─ 6. Write to tantivy
│     Add document with ALL fields:
│       body (full content, indexed, not stored)
│       summary (indexed + stored)
│       all semantic KV fields (indexed + stored)
│       significance, created_date, source_label, content_hash
│     Commit after batch.
│
├─ 7. Write to hot/
│     {artifact_id}.json with full content + metadata.
│     Plaintext. Claude can Read this directly.
│
└─ 8. Update state.json
      Record file path + mtime + content_hash.
```

Token cost of ingest: ZERO. This runs in a background process. No LLM involved. No API calls. Pure local computation.

---

## The Search Pipeline (what happens when the AI asks a question)

```
Query arrives (via CLI or MCP)
│
├─ 1. Synonym expansion
│     Tokenize query.
│     For each token, check synonym map (50+ groups).
│     "which database did we pick" becomes:
│     "(database OR db OR rdbms OR postgresql ...) (pick OR chose OR decided ...)"
│
├─ 2. tantivy query
│     Parse expanded query.
│     Search across ALL indexed fields:
│       body, summary, technology, category, action, domain, entities
│     BM25 scoring with stemming.
│     Date range filter if --after/--before provided.
│     Over-fetch 3x limit to allow for supersession filtering.
│
├─ 3. Supersession filter
│     For each result, check supersession index.
│     If superseded AND the superseding artifact is in results:
│       → Hide the old one (or flag + penalize 0.3x score)
│     If superseding artifact NOT in results:
│       → Keep the old one (better than nothing) but flag it
│
├─ 4. Return stored fields
│     For each result: summary, significance, created_date,
│     source_label, content_hash, superseded flag.
│
│     NO Parquet access. NO file reads. NO decompression.
│     Everything comes from tantivy stored fields.
│
└─ Token cost: ~200 tokens for 10 results (summaries only)
   vs ~10,000-80,000 tokens for grep+read cascade
```

---

## The Tier System (what happens when data ages)

```
myelin8 run (compaction phase)
│
├─ 1. Scan hot/ directory
│     For each .json file:
│       Check age (file mtime vs now)
│       Check idle (last_accessed vs now)
│       Check significance
│
├─ 2. Eligibility
│     Both age AND idle must exceed thresholds.
│     High significance delays eligibility.
│     Pinned artifacts are never eligible.
│
│     Default thresholds:
│       hot_age_hours: 48 (2 days)
│       hot_idle_hours: 24 (1 day)
│
├─ 3. Batch eligible artifacts
│     Collect all eligible hot files.
│     If none → done.
│
├─ 4. Write WAL manifest
│     wal/pending-{timestamp}.json lists:
│       - artifacts being compacted
│       - target Parquet filename
│     If process crashes, next run reads WAL
│     and either completes or rolls back.
│
├─ 5. Write Parquet batch
│     Write to store/.tmp/{filename}.parquet
│     zstd-9 compression on all columns.
│     Content, summary, metadata — everything in one file.
│     Atomic rename to store/{filename}.parquet
│
├─ 6. Verify Parquet integrity
│     Read back each row.
│     Recompute SHA-256 on content column.
│     Compare to stored content_hash.
│     If ANY mismatch → abort, keep hot files, delete Parquet.
│
├─ 7. Move hot files to .recycled/
│     NOT delete. Move.
│     .recycled/ files kept for 7 days.
│     If Parquet corruption is discovered later,
│     the hot files are still recoverable.
│
├─ 8. Clean .recycled/
│     Delete files older than recycle_days.
│
└─ 9. Delete WAL manifest
      Compaction complete. WAL cleaned.
```

### Crash recovery

```
Scenario: process killed during step 5 (Parquet write)
  → .tmp/ file exists, no rename happened
  → Next run: detect WAL manifest
  → Delete incomplete .tmp/ file
  → Hot files still exist (never moved)
  → Re-attempt compaction

Scenario: process killed during step 7 (hot → .recycled/)
  → Parquet exists (rename succeeded)
  → Some hot files moved, some not
  → Next run: detect WAL manifest
  → For each artifact in manifest:
    - If in Parquet AND in hot/ → move to .recycled/
    - If in Parquet AND in .recycled/ → already done
    - If NOT in Parquet → leave in hot/ (retry next run)
  → Clean WAL

Scenario: Parquet corruption discovered after compaction
  → myelin8 verify detects hash mismatch
  → User runs: myelin8 recall {artifact_id}
  → Recall checks .recycled/ first (within 7-day window)
  → If found in .recycled/ → restore to hot/
  → If .recycled/ expired → data lost (report to user)
```

---

## The Recall Pipeline (what happens when full content is needed)

```
myelin8 recall {artifact_id}
│
├─ 1. Check hot/ first
│     If {artifact_id}.json exists → return content directly.
│     Fast path. No Parquet access.
│
├─ 2. Check .recycled/
│     If {artifact_id}.json exists in .recycled/ → restore to hot/.
│     This handles the "just compacted, need it back" case.
│
├─ 3. Search Parquet files
│     List all .parquet files in store/.
│     For each file: scan artifact_id column.
│     When found: read content column (column-selective).
│
├─ 4. Verify integrity
│     SHA-256(content) == stored content_hash?
│     PASS → return content.
│     FAIL → warn user, return content with drift flag.
│
├─ 5. Write to hot/
│     {artifact_id}.json recreated in hot/.
│     Claude can now Read it directly.
│
├─ 6. Reset timestamps
│     last_accessed = now
│     Prevents immediate re-compaction.
│     Hebbian boost: significance += 0.05 (capped at 1.0)
│
└─ 7. Update tantivy
      Mark artifact as hot tier.
      (tantivy doesn't track tiers currently —
       this is metadata in the stored fields)
```

---

## The MCP Integration (how Claude connects)

### Configuration

```json
// ~/.claude/settings.json (or project settings)
{
  "mcpServers": {
    "myelin8": {
      "command": "/usr/local/bin/myelin8",
      "args": ["mcp-serve"]
    }
  }
}
```

Claude Code launches myelin8 as a subprocess. Communication via stdin/stdout JSON-RPC.

### Tools exposed

```
memory_search(query, after?, before?, limit?)
  → Returns: [{artifact_id, summary, significance, created_date, source_label, score, superseded}]
  → Token cost: ~200 for 10 results

memory_recall(artifact_id)
  → Returns: {content, integrity_status, size}
  → Token cost: proportional to content size

memory_status()
  → Returns: {hot_count, parquet_count, parquet_bytes, index_docs, index_terms, sources}
  → Token cost: ~50

memory_ingest(content, label)
  → Indexes a note directly from the AI
  → Returns: {artifact_id, content_hash}
  → Token cost: ~30
```

### Claude Code CLAUDE.md instruction

```markdown
## Memory

Myelin8 is connected via MCP. Use it for historical context:

- For anything from a previous session: call memory_search first
- For full content after finding a relevant result: call memory_recall
- NEVER try to Read .parquet files — they are compressed storage
- Today's daily log and MEMORY.md are still readable directly (hot tier)
- When the user says "remember this": call memory_ingest
```

### SessionStart hook update

```python
# memory-load.py additions:
import subprocess
import json

# After loading today + yesterday daily logs (existing behavior):
# Inject myelin8 context summary for the last 7 days
try:
    result = subprocess.run(
        ["myelin8", "search", "--recent", "7d", "--limit", "5", "--json"],
        capture_output=True, text=True, timeout=5
    )
    if result.returncode == 0:
        memories = json.loads(result.stdout)
        if memories:
            context = "\n".join(
                f"- [{m['created_date']}] {m['summary']}"
                for m in memories
            )
            parts.append(f"=== RECENT MEMORIES (via myelin8) ===\n\n{context}")
except Exception:
    pass  # fail silently, never block session start
```

---

## Build Order

Not everything gets built at once. Each phase validates the previous one before adding complexity.

### Phase 1: Make compaction real (1-2 days)

What: hot files age into Parquet. Verify round-trip. WAL for crash safety.

```
Build:
  - state.json (mtime tracking, skip already-seen files)
  - WAL manifest (pending-{timestamp}.json)
  - .recycled/ management (move, not delete, with TTL cleanup)
  - Parquet post-write verification (read back, check all hashes)
  - Progress output (indicatif bars on myelin8 run)

Test:
  - Create 100 test memories
  - Set hot_age_hours to 0 (force immediate compaction)
  - Run myelin8 run → verify all 100 in Parquet
  - Recall each → verify SHA-256 matches
  - Kill -9 during compaction → restart → verify no data loss
  - Corrupt a Parquet file → verify myelin8 verify catches it
  - Verify search still works during and after compaction

Ship when:
  - 100/100 round-trip integrity pass
  - Crash recovery works for all three crash scenarios
  - myelin8 verify reports corruption accurately
```

### Phase 2: Wire search quality (1 day)

What: synonym expansion actually used in search path. Measure quality.

```
Build:
  - Wire semantic::expand_query() into index.rs search method
  - Add --json output flag for CLI (machine-readable results)
  - Add search --recent flag (shorthand for date range from now)

Test:
  - 25 paraphrase queries (known answers, synonym-dependent)
  - Measure: P@1 (is the correct answer the top result?)
  - Compare: with expansion vs without
  - Target: P@1 > 0.80 with expansion

Ship when:
  - Synonym expansion measurably improves search quality
  - No regression on exact-match queries
```

### Phase 3: Connect Claude (1 day)

What: MCP server tested with real Claude Code session. CLAUDE.md written. Hook updated.

```
Build:
  - Add myelin8 to Claude Code MCP config
  - Write CLAUDE.md memory instruction
  - Update SessionStart hook to inject myelin8 context
  - Add --json flag to all CLI commands (MCP and hook both need it)

Test:
  - Start Claude Code session with myelin8 connected
  - Ask "what happened last week?" → does Claude use memory_search?
  - Ask "tell me more about X" → does Claude use memory_recall?
  - Say "remember this decision" → does Claude use memory_ingest?
  - Verify: Claude never tries to Read .parquet files
  - Verify: token count for historical queries drops measurably

Ship when:
  - Claude correctly routes to myelin8 for historical queries
  - Claude correctly uses Read for hot/current files
  - No session failures caused by myelin8
```

### Phase 4: Pin + significance persistence (half day)

What: user can pin important memories. Significance scores persist across runs.

```
Build:
  - myelin8 pin stores significance=1.0 in tantivy + state
  - myelin8 unpin reverts to computed significance
  - Hebbian boost on recall: significance += 0.05 per access
  - Significance decay on compaction: unused memories lose 0.01/week

Test:
  - Pin a memory → verify it resists compaction
  - Recall a memory 5 times → verify significance increases
  - Leave a memory untouched for simulated weeks → verify it decays

Ship when:
  - Pinned memories never compact
  - Significance persists across myelin8 restarts
```

### Phase 5: Distribution (half day)

What: users can install without building from source.

```
Build:
  - GitHub Actions: build macOS (arm64 + x86_64) + Linux binaries
  - GitHub Releases: upload binaries on tag push
  - Homebrew formula (tap: qinnovates/tap)
  - Update README install instructions

Test:
  - Download binary on clean machine
  - myelin8 init → add → run → search works without Rust toolchain

Ship when:
  - brew install qinnovates/tap/myelin8 works
  - Binary size < 15MB
```

### Phase 6: Encryption layer (future, after all above is stable)

What: encrypt Parquet files at rest. Decrypt transparently on recall/search.

```
Build:
  - Encrypt whole Parquet files with AES-256-GCM
  - Key derivation from user passphrase or system keychain
  - Decrypt on recall (transparent to search — tantivy index is separate)
  - myelin8 lock / myelin8 unlock for index encryption

NOT building:
  - ML-KEM-768 PQC (defer to v3 — overkill for v2 launch)
  - Per-artifact keys (defer — whole-file encryption is simpler)
  - Rust sidecar isolation (defer — single binary is the priority)

Test:
  - Encrypt → restart → search still works (index not encrypted)
  - Encrypt → recall → content decrypted correctly
  - Wrong passphrase → graceful error, not crash
  - Verify encryption doesn't break Claude's MCP access
```

---

## Token Accounting

Every operation has a token cost. The system must reduce total token consumption, not just shift it.

### Current cost (without myelin8)

```
Historical query: 15,000-80,000 tokens
  Glob:        200 tokens
  Grep:        500 tokens
  Read (×5):   5,000-50,000 tokens
  Subagent:    5,000-20,000 tokens
  Synthesis:   1,000-5,000 tokens

Per-session overhead: 8,000-10,000 tokens
  CLAUDE.md:   3,000 tokens
  Rules:       3,000 tokens
  Memory:      2,000-4,000 tokens (daily logs, MEMORY.md)
```

### Target cost (with myelin8)

```
Historical query: 200-2,500 tokens
  memory_search:  200 tokens (summaries)
  memory_recall:  500-2,000 tokens (if full content needed)

Per-session overhead: 8,500-10,500 tokens
  CLAUDE.md:      3,000 tokens (unchanged)
  Rules:          3,000 tokens (unchanged)
  Memory:         2,000-4,000 tokens (unchanged — hot files still direct)
  myelin8 inject: 500 tokens (SessionStart hook summary of last 7 days)
```

### Net savings per session

```
Best case (5 historical queries avoided):
  Old: 5 × 20,000 = 100,000 tokens
  New: 5 × 500 = 2,500 tokens
  Saved: 97,500 tokens (39x reduction)

Average case (2 historical queries):
  Old: 2 × 15,000 = 30,000 tokens
  New: 2 × 1,000 = 2,000 tokens + 500 (hook inject)
  Saved: 27,500 tokens (12x reduction)

Worst case (0 historical queries, session is all new work):
  Old: 0 additional tokens
  New: 500 tokens (hook inject overhead)
  Cost: +500 tokens (acceptable — 0.5% of a 100K session)
```

### Disk accounting

```
Per artifact:
  Hot:     ~5KB (JSON with metadata)
  Parquet: ~200 bytes (in batched file, zstd-9)
  Index:   ~50 bytes (tantivy entry)

At scale:
  1K artifacts:   5MB hot → 200KB Parquet + 50KB index
  10K artifacts:  50MB hot → 2MB Parquet + 500KB index
  100K artifacts: 500MB hot → 20MB Parquet + 5MB index

Hot files are transient (compact after 48 hours).
Steady state disk usage = Parquet + index.
10 years of daily use (~36K artifacts): ~8MB Parquet + ~2MB index.
```

### Compute accounting

```
Ingest (per artifact):
  SHA-256:           <0.1ms
  Keyword extraction: <1ms
  Semantic KV:       <1ms
  SimHash:           <0.1ms
  tantivy write:     <1ms
  Total:             <3ms per artifact

Search:
  Synonym expansion:  <0.1ms
  tantivy query:      <3ms
  Supersession check: <0.1ms
  Total:              <5ms per query

Compaction (per batch):
  Read hot files:     <1ms per file
  Write Parquet:      <5ms per batch
  Verify hashes:      <2ms per batch
  Total:              <10ms for 50 artifacts

Recall:
  Parquet column read: <1ms
  SHA-256 verify:      <0.1ms
  Write to hot:        <1ms
  Total:               <3ms per recall
```

---

## What this is NOT

This is not a RAG system. It does not chunk text into 500-token segments and embed them in a vector database. It does not rely on cosine similarity to find "semantically similar" content. It does not require a running database server, a Docker container, or an API key.

This is not an AI memory product that replaces the LLM's context window. The context window still exists. The LLM still has finite attention. What this does is make every token in that context window count by putting the right information in, instead of flooding it with grep results hoping the relevant paragraph is somewhere in the noise.

This is not a general-purpose search engine. It is purpose-built for one thing: making AI coding assistants remember what happened in previous sessions without burning tokens and compute re-reading old files. The SIEM architecture is the mechanism. The token savings are the product.

---

## Measurement

Before any of this ships to users, these numbers must be real:

1. **Round-trip integrity:** 1,000 artifacts through Parquet and back. Target: 0 hash mismatches.
2. **Search quality (P@1):** 25 queries with known answers. Target: >0.80 with synonym expansion.
3. **Token savings:** 10 real historical queries in a Claude Code session. Measure actual tokens with and without myelin8. Target: >5x average reduction.
4. **Crash recovery:** Kill -9 at every step of compaction. Restart. Target: 0 data loss.
5. **Concurrent access:** MCP server reading while CLI writes. Target: 0 lock contention on readers.

If any measurement misses its target, fix the code, don't adjust the target.
