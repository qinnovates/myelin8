# Architecture

A SIEM-inspired memory engine for AI assistants. Tiered compression, hybrid search, significance scoring, and integrity verification — designed to reduce token usage and compute cost while keeping years of AI memory searchable.

---

## Table of Contents

- [Design Philosophy](#design-philosophy)
- [The SIEM Analogy](#the-siem-analogy)
- [Data Flow](#data-flow)
- [Registration and Indexing](#registration-and-indexing)
- [Significance Scoring](#significance-scoring)
- [Tier Transitions](#tier-transitions)
- [Compression Pipeline](#compression-pipeline)
- [Search and Retrieval](#search-and-retrieval)
- [Hybrid Search](#hybrid-search)
- [Merkle Tree](#merkle-tree)
- [Recall](#recall)
- [Embedding Architecture](#embedding-architecture)
- [CoGraph (Associative Recall)](#cograph-associative-recall)
- [Encryption (Opt-In)](#encryption-opt-in)
- [File Layout](#file-layout)
- [Protected Paths](#protected-paths)

---

## Design Philosophy

Two principles drive every design decision:

1. **Don't recompute what you can store and look up.** Why recompute what the full digits of pi is if you have the answer written on a chalkboard? Rather than recomputing pi from the actual formula, we just know 3.14, it's close enough, it's an estimate. Every retrieval path uses precomputed indexes, summaries, and keyword maps — never raw file scanning.

2. **Search the index, not the data.** Large data warehouses like SIEMs use indexing, compression, and KV stores because when you're dealing with massive data, you can't grep through everything. The index tells you WHERE the answer is. You only open the actual data when you know it's there.

---

## The SIEM Analogy

This engine is modeled after how Splunk, Elasticsearch, and Snowflake handle petabyte-scale log data — not after how RAG systems handle text chunks.

| SIEM Component | What It Does | Myelin8 Equivalent |
|---|---|---|
| **Indexer** | Ingests raw logs, compresses, writes to time-partitioned buckets | `myelin8 add` + `myelin8 run` — user defines sources, engine indexes and compresses |
| **Buckets** (hot/warm/cold/frozen) | Time-partitioned storage, decreasing access speed | Tiered memory: recent = full text, old = compressed, oldest = columnar |
| **tsidx** (inverted index) | Knows which terms exist in which bucket WITHOUT reading raw data | Semantic index — keywords, summaries, hashes per artifact |
| **Bloom filters** | Probabilistic check: "does this bucket contain term X?" Skip buckets that can't match | Merkle tree — existence check + partition routing without decompression |
| **Search head** | Coordinates queries across indexers, merges results | Hybrid search — FTS + semantic + RRF fusion |
| **Cluster master** | Manages tier locations, handles decompression transparently | Engine — the AI never sees compressed data, engine returns plaintext |
| **SmartStore** | Remote cold/frozen storage, cache locally on access | Frozen Parquet tier, `recall` promotes back to hot |

The key principle from SIEMs: **the search head never reads raw data.** It queries indexes. It checks filters. It only opens actual data when it knows the answer is there. This is the opposite of what RAG systems do (embed everything, search everything, hope cosine similarity finds it).

---

## Data Flow

```
User defines sources:
    myelin8 add ~/projects/_memory/ --label memory
    myelin8 add ~/projects/_swarm/ --label swarm

         │
    myelin8 run (processes all registered sources)
         │
         ▼
  ┌─ REGISTRATION ──────────────────────────────────┐
  │  Content hash │ keyword extraction │ summary     │
  │  significance scoring │ embedding (if enabled)   │
  │  SimHash fingerprint (near-duplicate detection)  │
  └──────────────────────────────────────────────────┘
         │
    myelin8 run (significance × age × idle thresholds)
         │
         ▼
  ┌─ TIER TRANSITIONS ─────────────────────────────┐
  │  HOT ──significance decays──▶ WARM             │
  │       ──1 week + idle──▶     WARM              │
  │  WARM ──1 month──▶ COLD ──3 months──▶ FROZEN   │
  │                                                 │
  │  High-significance memories RESIST decay.       │
  │  Accessed memories BOOST back toward hot.       │
  └─────────────────────────────────────────────────┘
         │
    myelin8 search / myelin8 context
         │
         ▼
  ┌─ RETRIEVAL (no decompression needed) ──────────┐
  │  keyword lookup (FTS) ──┐                      │
  │  semantic search ───────┼──▶ RRF fusion        │
  │  Merkle partition check ┘   → return summaries │
  └────────────────────────────────────────────────┘
         │
    myelin8 recall (only when full content needed)
         │
         ▼
  ┌─ RECALL ─────────────────────────────────────┐
  │  decompress (pipeline reversal)              │
  │  → integrity check (content hash)            │
  │  → decrypt if encrypted (Rust sidecar)       │
  │  → return to hot tier                        │
  └──────────────────────────────────────────────┘
```

---

## Registration and Indexing

When an artifact is first discovered (`myelin8 run`), the engine builds a complete index entry BEFORE any compression. This is the "write once, search many" principle — all the expensive work happens at ingest, not at query time.

| Step | What's Created | Purpose |
|------|---------------|---------|
| Content hash (SHA-256) | Integrity fingerprint | Dedup, tamper detection, Merkle leaf |
| Keyword extraction | Top 30 terms by frequency | Full-text search without reading content |
| Summary | First heading + paragraph (md), field names + count (jsonl) | Token-efficient retrieval — return summary, not full content |
| Significance score | Heuristic importance weight (0.0 - 1.0) | Determines tier decay rate |
| SimHash fingerprint | 256-bit semantic fingerprint | Near-duplicate detection (Hamming distance) |
| Embedding (if enabled) | 384-dim vector via all-MiniLM-L6-v2 | Semantic search for fuzzy/paraphrased queries |
| HNSW insert (if enabled) | Nearest-neighbor graph entry | Fast approximate vector search |
| Timestamps | Created, last accessed, last modified | Age + idle calculations for tiering |

Gzip-compressed files (`.jsonl.gz` from Claude subagents) are decompressed in memory for indexing. The file on disk stays compressed.

---

## Significance Scoring

Human memory is selective because it's useful. We remember corrections, decisions, and novel encounters. We forget routine operations. This engine applies the same principle.

At ingest, each artifact is scored:

| Signal Detected | Score | Rationale |
|---|---|---|
| User correction ("no, not that", "stop", rephrasing) | 0.95 | Corrections change behavior — highest learning signal |
| User said "remember this" | 1.0 | Explicit instruction to retain |
| Contains decision language ("we decided", "because", "going with") | 0.85 | Decisions are the most-queried memories |
| Architectural/design choice | 0.80 | Long-lived, frequently referenced |
| Error + fix pattern | 0.60 | Valuable but time-limited |
| New concept first encountered | 0.50 | Novelty signal — may or may not matter later |
| Routine file read / grep output | 0.10 | Almost never queried again |
| Tool call boilerplate | 0.05 | Never queried — skip indexing entirely |

**Hebbian reinforcement:** When a memory is accessed via `search` or `recall`, its significance score increases. Memories that get referenced stay hot. Memories that are never referenced decay through tiers faster.

**Supersession:** When a new decision contradicts an older one ("we changed from PostgreSQL to SQLite"), the older decision's significance drops and the new one inherits the higher score.

This is significance *heuristics* — pattern matching on signal characteristics, not cognitive comprehension. Imperfect but better than treating every token equally.

---

## Tier Transitions

Transitions are driven by **significance × age × idle time**, not age alone.

| Transition | Base Age | Base Idle | Modified By |
|---|---|---|---|
| Hot → Warm | 1 week | 3 days | High significance delays transition |
| Warm → Cold | 1 month | 2 weeks | Accessed memories boost back |
| Cold → Frozen | 3 months | 1 month | Pinned memories never freeze |

A critical decision from 6 months ago can stay in hot tier if it's still being referenced. A routine grep from yesterday can skip warm and go straight to cold if its significance score is below threshold.

All thresholds configurable in `config.json`.

### Warm transition walkthrough

```
File: session-2025-09-12.jsonl (1.5 MB, significance: 0.3)
  Age: 8 days (> 1 week), Idle: 5 days (> 3 days)
  Significance too low to resist → HOT → WARM

  ├─ Stage 1: JSON minification
  │   Strip whitespace, normalize formatting
  │   1.5 MB → 1.0 MB (33% removed)
  │
  ├─ Stage 2: zstd level 3 compression
  │   1.0 MB → 340 KB (4.4x from original)
  │
  ├─ Embedding truncated (if enabled):
  │   384d float32 → 256d float32 (Matryoshka — drop last 128 dims)
  │
  └─ Index entry updated: tier=warm, compressed_path=session.jsonl.zst
```

### Cold transition walkthrough

```
Warm artifact: session.jsonl.zst (340 KB)
  Age: 5 weeks, Idle: 3 weeks → WARM → COLD

  ├─ Decompress warm zstd back to raw content
  │   340 KB → 1.0 MB (minified JSON)
  │
  ├─ Stage 1: Boilerplate stripping
  │   The 3,000-token system prompt repeated in every session
  │   Replaced with: BOILERPLATE_REF:a3f7b2e1 (64 bytes)
  │   Original stored once in ~/.myelin8/boilerplate/
  │   Content reduced by 40-70%
  │   1.0 MB → 400 KB
  │
  ├─ Stage 2: JSON minification (cleanup pass)
  │
  ├─ Stage 3: Dictionary-trained zstd level 9
  │   Dictionary (112 KB) trained on your sessions knows your
  │   JSON schema, common tokens, tool call formats.
  │   Only compresses what's unique to this session.
  │   400 KB → 150 KB (10x from original)
  │
  ├─ Embedding truncated (if enabled):
  │   256d float32 → 128d int8 quantized (4x smaller)
  │
  └─ Index entry updated: tier=cold
```

### Frozen transition walkthrough

```
Cold artifact: session.jsonl.cold.zst (150 KB)
  Age: 4 months, Idle: 6 weeks → COLD → FROZEN

  ├─ Decompress cold zstd (using trained dictionary)
  │
  ├─ Columnar Parquet conversion
  │   JSONL rows transposed to columns:
  │
  │   role column:      ["user","assistant","user",...]
  │     → cardinality 2 → run-length encoding → ~0 bytes
  │
  │   timestamp column: [1710000000, 1710000060, 1710000120,...]
  │     → monotonically increasing → delta encoding → ~0 bytes
  │
  │   content column:   actual conversation text
  │     → dictionary encoding (repeated phrases) + zstd-19
  │
  │   150 KB → 50 KB (30x from original)
  │
  │   Column-selective reads: "give me all decisions" reads
  │   only the content column with type=decision filter.
  │   Never touches timestamps, roles, or tool calls.
  │
  ├─ Embedding compressed (if enabled):
  │   128d int8 → 48-byte packed binary (Product Quantization)
  │
  └─ Index entry updated: tier=frozen
```

---

## Compression Pipeline

Each tier applies progressively more aggressive compression:

```
HOT    ████████████████████████████████████████  1,500 KB   1x     instant
WARM   ████████████                              340 KB    4-5x    ~10ms
COLD   ██████                                    150 KB    8-12x   ~200ms
FROZEN ██                                        50 KB    20-50x   ~2s
```

| Tier | Techniques Applied | Ratio |
|------|---|---|
| Hot | None (original plaintext) | 1x |
| Warm | JSON minification + zstd-3 | 4-5x |
| Cold | Boilerplate stripping + minification + dictionary-trained zstd-9 | 8-12x |
| Frozen | Columnar Parquet + dictionary encoding + RLE + delta + zstd-19 | 20-50x |

The dictionary (`~/.myelin8/compression.dict`, 112 KB) is trained on representative sessions and reused across all cold/frozen compressions. It knows your JSON schema, so it only compresses what's unique to each session.

---

## Search and Retrieval

Search NEVER decompresses data. The semantic index (built at registration time) contains everything needed to answer queries: keywords, summaries, significance scores, content hashes.

```
Query: "what did we decide about the database?"
  │
  ├─ FTS keyword lookup: "decide" + "database"
  │   → 3 matches (artifacts a3f8, b7d1, c9f2)
  │
  ├─ Semantic search (if embeddings enabled):
  │   → 5 matches by cosine similarity
  │
  ├─ RRF fusion: merge both result sets
  │   → artifacts appearing in BOTH rank highest
  │   → b7d1 ranks #1 (keyword match + semantic match)
  │
  └─ Return: summary of b7d1 (200 tokens)
     NOT the full 10,000-token session
```

**Token savings:** 200 tokens returned vs 10,000+ tokens from reading the raw session. The AI gets the answer. The context window stays clean.

---

## Hybrid Search

No single search method handles every query type. Different queries fail in different ways:

| Query Type | FTS Catches | FTS Misses |
|---|---|---|
| "database migration" | Exact term match | Paraphrased as "schema changes" |
| "that refactor from last week" | "refactor" matches | "that" and "last week" are noise |

| Query Type | Semantic Catches | Semantic Misses |
|---|---|---|
| "schema changes" | Conceptually similar to "database migration" | Might also return "API schema" (wrong context) |
| "meeting with Adam" | Related to "Adam" discussions | Can't distinguish which Adam |

**Reciprocal Rank Fusion (RRF)** combines results from FTS and semantic search. An artifact that ranks well in BOTH methods gets boosted. An artifact that only ranks in one method ranks lower. This covers more query variations than either method alone.

```python
# RRF formula (Cormack et al., 2009)
# k=60 dampens the impact of high-ranking outliers
score(artifact) = Σ 1 / (k + rank_in_method_i)
```

Search cascade order:
1. **FTS keyword lookup** — fast, exact, always available
2. **HNSW vector search** — semantic, approximate (if embeddings enabled)
3. **RRF fusion** — combines 1 + 2
4. **Brute-force cosine** — fallback if HNSW unavailable
5. **Spreading activation** — CoGraph traversal for related artifacts (if enabled)

---

## Merkle Tree

Dual-purpose: integrity verification AND search acceleration.

### Integrity (Anti-Hallucination)

Every artifact gets a leaf in a SHA3-256 binary hash tree. The root hash covers all artifacts across all four tiers.

```
                  Root: 061e... (SHA3-256)
                    /              \
             Hash AB                Hash CD
            /      \               /      \
      Hash A      Hash B     Hash C      Hash D
        |           |          |           |
   Session 1   Session 2   Session 3   Session 4
    (hot)       (warm)      (cold)     (frozen)
```

When the AI claims "we decided X in Session 3," produce a Merkle proof — a path of hashes from Session 3's leaf to the root. If the proof verifies, the conversation is real. If it doesn't, the AI fabricated it.

### Search Acceleration

The Merkle tree also accelerates search by enabling:

1. **Existence checks without decompression.** Before searching a cold/frozen tier, check if the content hash exists in the tree. If not, skip that tier entirely. O(log n).

2. **Change detection for incremental indexing.** When the root hash changes, walk the tree to find which subtree changed. Only re-index the changed branch.

3. **Partition-aware routing.** Branches organized by time/project namespace. A query scoped to "project X, last 30 days" traverses only the relevant subtree.

The tree runs in compiled Rust (`myelin8-vault` sidecar) for performance. The sidecar also maintains an inverted keyword index mapping keyword hashes to content hashes — enabling term lookups without exposing plaintext keywords.

---

## Recall

When the summary isn't enough and full content is needed:

```bash
myelin8 recall ~/.claude/projects/.../session-2025-09-12.jsonl
```

1. Locate compressed artifact by content hash
2. Decompress (reverse the compression pipeline for that tier)
3. Decrypt if encrypted (Rust sidecar)
4. Verify integrity (content hash matches registered hash)
5. Promote back to hot tier
6. Return plaintext content

Cold recall: ~200ms. Frozen recall: ~2s. The latency is acceptable because recall is rare — search handles 90%+ of queries via summaries.

---

## Embedding Architecture

Optional (`pip install myelin8[embeddings]`). Not required for FTS-only operation.

### Matryoshka Embeddings

One embedding model (all-MiniLM-L6-v2, 384 dimensions), four resolutions per tier:

| Tier | Dimensions | Format | Size per Vector | Purpose |
|------|-----------|--------|----------------|---------|
| Hot | 384 | float32 | 1,536 bytes | Full precision search |
| Warm | 256 | float32 | 1,024 bytes | Good precision, 33% smaller |
| Cold | 128 | int8 | 128 bytes | Quantized, 92% smaller |
| Frozen | 64 | binary packed | 48 bytes | Coarse matching, 97% smaller |

Matryoshka ("Russian doll") embeddings are truncated, not retrained. The first N dimensions of a 384-dim embedding ARE a valid N-dim embedding. This is a property of how the model was trained — each prefix is a self-contained representation at lower resolution.

### HNSW Vector Index

Hierarchical Navigable Small World graphs for approximate nearest neighbor search. Per-tier graphs with tuned parameters:

- `M=16` — max connections per node
- `ef_construction=200` — build-time accuracy
- `ef_search=50` — query-time accuracy

O(log n) search vs O(n) brute force. Deferred for MVP — brute-force cosine is fast enough under 50K artifacts.

---

## CoGraph (Associative Recall)

Co-occurrence tracking + spreading activation. When artifacts appear together in sessions, they get edges in a graph. When you search for one artifact, related artifacts surface automatically.

**Implementation:** Rust sidecar (`cograph.rs`). PPMI-weighted edges, BFS spreading activation (depth cap: 3, top_k limited for DoS prevention). In-memory only — never persisted to disk.

**Status:** Implemented, being evaluated. Deferred from MVP until usage data shows whether associative recall has measurable value.

---

## Encryption (Opt-In)

Available via `pip install myelin8[secure]`. Not required. Not in the default install.

When enabled:
- **ML-KEM-768 + X25519** hybrid key encapsulation (NIST FIPS 203, post-quantum safe)
- **AES-256-GCM** authenticated encryption per artifact
- **Per-tier keypairs** — compromise warm, cold stays safe
- **Rust sidecar** handles all crypto — private keys never enter Python
- **macOS Keychain** integration for key storage (Touch ID)

See `docs/KEY-STORAGE-GUIDE.md` for setup.

---

## File Layout

```
~/.myelin8/
├── config.json           # User-defined sources (via `myelin8 add`), tier thresholds, feature flags
├── artifact-registry.json # Per-artifact metadata (hash, tier, timestamps, significance)
├── semantic-index.json    # Keywords, summaries per artifact (search index)
├── compression.dict       # Trained zstd dictionary (112 KB)
├── embeddings/            # Per-tier .npy files (if embeddings enabled)
├── hnsw-*.bin             # Per-tier HNSW graphs (if enabled)
└── (frozen .parquet files stored alongside compressed artifacts)
```

---

## Protected Paths

Myelin8 indexes AI assistant directories but NEVER modifies them:

```python
PROTECTED_PATHS = [
    Path.home() / ".claude",
    Path.home() / ".cursor",
    Path.home() / ".config" / "github-copilot",
]
```

These directories are managed by their respective AI assistants. Myelin8 can read and index them for search. It cannot compress, encrypt, or delete files inside them. This is enforced in `engine.py` via `_is_protected()` — the guard runs before every tier transition.
