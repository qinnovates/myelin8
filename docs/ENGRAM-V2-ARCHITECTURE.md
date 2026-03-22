# Engram v2 Architecture — Brain-Modeled Real-Time AI Memory

*Drafted 2026-03-21. Kevin Qi + Claude Opus 4.6.*

## The Problem With v1

Engram v1 models the brain's storage tiers correctly. But it models the brain's **retrieval** wrong.

When you remember something, your brain doesn't:
1. Search an index to find which region stores it
2. Decompress the entire memory
3. Load it into working memory
4. Then use it

Your brain does:
1. A **cue** activates related neurons (spreading activation)
2. **Partial reconstruction** — you get the gist before the details
3. Details fill in on demand (or don't — that's normal)
4. **Priming** — related memories pre-activate, so the NEXT recall is faster

Engram v1 makes Claude do the file system version of step 1-2-3-4. It works, but it's slow for cold/frozen tiers and it's all-or-nothing — you either get the full artifact or just the summary. The brain doesn't work that way.

---

## The Brain's Retrieval Model

### How the Hippocampus Actually Works

The hippocampus doesn't store memories. It stores **pointers to memories** — pattern-separated index codes that can reconstruct the original experience from fragments stored across the neocortex.

| Brain Component | Current Engram | Engram v2 |
|----------------|---------------|-----------|
| **Working memory (PFC)** | Hot tier (full content) | Same — current session context |
| **Hippocampal index** | Semantic index (keywords + embeddings) | **Merkle-indexed semantic graph** — every summary has a proof |
| **Pattern completion** | `engram recall` (decompress whole artifact) | **Progressive reconstruction** — summary → sections → full, on demand |
| **Spreading activation** | None | **Activation graph** — recalling one artifact pre-warms related artifacts |
| **Memory consolidation** | `engram run` (compress by age) | **Active consolidation** — restructure, extract decisions, build cross-links |
| **Priming** | None | **Predictive pre-fetch** — session topic pre-warms likely artifacts |
| **Reconsolidation** | None | **Update on access** — recalled memories get re-indexed with new context |

### The Key Insight: Search IS Recall

In the brain, searching for a memory and recalling it are the same operation. The search cue triggers partial activation which IS the memory (at low fidelity). More attention = more detail fills in.

In Engram v1, search and recall are separate operations with different latency:
```
engram search "auth refactor"     → 10ms (returns summary)
engram recall ~/.claude/session.md → 500ms-5s (decompresses everything)
```

In Engram v2, they're the same operation with progressive fidelity:
```
engram recall "auth refactor"
  → 5ms:  matched 3 artifacts, returning verified summaries
  → 20ms: expanding top match with section headers + key decisions
  → 50ms: streaming relevant sections (only the auth parts, not the whole session)
  → full: complete decompressed artifact (only if explicitly requested)
```

---

## Architecture

### 1. Merkle-Verified Summaries (Trust Without Decompressing)

**Problem:** Claude gets a summary from the semantic index, but has no proof the summary accurately represents the compressed artifact. The summary could be stale, corrupted, or fabricated.

**Solution:** Every summary in the semantic index gets its own Merkle leaf, alongside the artifact's content leaf. The summary leaf is linked to the content leaf. Claude can verify the summary is authentic via Merkle proof without decompressing the content.

```
Merkle Tree:
          Root
         /    \
   Content     Summaries
   /    \      /      \
  Art1  Art2  Sum1    Sum2
               ↓       ↓
           (linked to Art1, Art2)
```

**Result:** Claude trusts the summary. No decompression needed for 80% of recall operations. Merkle proof verifies in microseconds via the Rust sidecar.

### 2. Progressive Reconstruction (Partial Recall)

**Problem:** Cold/frozen recall is all-or-nothing. Decompress 50KB to find the one 200-byte decision you need.

**Solution:** Structure artifacts with a header section that survives compression:

```
Artifact layout (pre-compression):
┌─────────────────────────────────┐
│ HEADER (never stripped)         │
│   - Summary (1-2 sentences)     │
│   - Key decisions (bullet list) │
│   - Section index               │
│   - Merkle leaf hash            │
├─────────────────────────────────┤
│ SECTIONS (independently         │
│   addressable after compress)   │
│   [0] Context and background    │
│   [1] Discussion                │
│   [2] Decisions and outcomes    │
│   [3] Code changes              │
│   [4] Open questions            │
└─────────────────────────────────┘
```

Compression pipeline change: use **zstd frame-level compression** — each section is a separate zstd frame within the same file. Decompress frame 0 (header) = instant. Decompress frame 2 (decisions) = skip frames 0-1. No need to decompress the whole artifact.

```python
# v1: all or nothing
content = engram.recall(path)  # 500ms, returns 50KB

# v2: progressive
header = engram.recall(path, depth="header")    # 5ms, returns 500B
decisions = engram.recall(path, section=2)       # 20ms, returns 2KB
full = engram.recall(path, depth="full")         # 500ms, returns 50KB
```

### 3. Activation Graph (Spreading Activation)

**Problem:** Recalling one artifact gives you no information about related artifacts. Claude has to search separately for each connection.

**Solution:** Build a weighted edge graph between artifacts based on:
- Shared keywords (weight = Jaccard similarity)
- Temporal proximity (sessions within 24h of each other)
- Explicit references (file paths mentioned in another session)
- Embedding cosine similarity (already computed, stored in HNSW)

```
Session about auth refactor
    ├── 0.8 → Session about API security (shared: "auth", "token", "middleware")
    ├── 0.6 → Session about database migration (same day, mentioned auth table)
    ├── 0.3 → Session about deployment (mentioned auth service restart)
    └── 0.1 → Session about UI redesign (low similarity, same week)
```

When Claude recalls the auth refactor session, Engram automatically returns the top-3 related sessions' summaries. No extra search needed. The graph pre-computes what's related — like the hippocampus pre-activating associated memories.

```python
result = engram.recall("auth refactor")
# Returns:
#   .content = the auth refactor session (progressive)
#   .related = [
#     {summary: "API security review", relevance: 0.8, proof: <merkle>},
#     {summary: "DB migration - auth tables", relevance: 0.6, proof: <merkle>},
#   ]
#   .proof = <merkle proof for this artifact>
```

### 4. Predictive Pre-Fetch (Priming)

**Problem:** Cold/frozen recall is slow because decompression happens on-demand. Claude waits 500ms-5s while the user waits.

**Solution:** At session start, analyze the conversation topic and pre-warm likely-needed artifacts.

```
SessionStart hook runs → engram context --query "current topic"
  → Top 5 cold/frozen artifacts by relevance score
  → Background: decompress headers + section indices for all 5
  → Store in a warm cache (in-memory, expires after session)
  → When Claude actually needs one: header already in memory, 0ms
```

This is exactly how the brain works. Walking into a kitchen primes food-related memories before you consciously think about cooking. The context (location/topic) triggers anticipatory activation.

### 5. Reconsolidation (Update on Access)

**Problem:** When Claude recalls a 3-month-old session about auth, the keywords and summary reflect what was important 3 months ago, not what's relevant now. The next search for auth-related content might rank it lower because the old index entry doesn't have today's terminology.

**Solution:** When an artifact is recalled, Engram re-indexes it with the current session's context:

```
Original keywords (3 months ago): ["auth", "middleware", "express", "jwt"]
Current session context: ["oauth2", "refresh tokens", "session management"]

After reconsolidation:
Updated keywords: ["auth", "middleware", "express", "jwt", "oauth2", "refresh tokens"]
Updated embedding: blended (0.7 * original + 0.3 * current context)
Updated summary: includes note about relevance to current oauth2 work
```

This mirrors how the brain's reconsolidation works: every time you recall a memory, it's subtly rewritten with your current context. The memory becomes easier to find in the future for similar contexts.

---

## Implementation Phases

### Phase 1: Verified Summaries + Progressive Recall
- [ ] Merkle leaf for every summary in semantic index
- [ ] `engram recall --depth header|section|full`
- [ ] Zstd frame-per-section compression in pipeline
- [ ] CLI returns Merkle proof with every recall
- [ ] Claude hook: verify proof before trusting recalled content

### Phase 2: Activation Graph
- [ ] Build edge graph from keyword Jaccard + temporal + embedding similarity
- [ ] Store as adjacency list in `activation-graph.json`
- [ ] `engram recall` returns top-3 related summaries automatically
- [ ] Graph updates on every `engram scan` (new sessions add edges)
- [ ] Prune edges below threshold (prevent graph explosion)

### Phase 3: Predictive Pre-Fetch
- [ ] SessionStart hook calls `engram prefetch --topic "..."`
- [ ] Background decompression of top-N cold/frozen headers
- [ ] In-memory warm cache (expires on session end)
- [ ] Measure: average recall latency before and after pre-fetch

### Phase 4: Reconsolidation
- [ ] On recall: blend current context into artifact's keywords and embedding
- [ ] Track "recall count" and "last recalled context" per artifact
- [ ] Frequently-recalled artifacts resist tier demotion (they're important)
- [ ] Implement "importance score" = f(recall_count, recency, relevance)

---

## Latency Targets

| Operation | v1 | v2 Target |
|-----------|-----|-----------|
| Search (summary) | 10ms | 5ms (Merkle-verified) |
| Recall header | N/A (all-or-nothing) | 5ms |
| Recall section | N/A | 20ms |
| Recall full (hot) | instant | instant |
| Recall full (warm) | 10ms | 10ms |
| Recall full (cold) | 500ms | 50ms (pre-fetched) or 200ms (frame-skip) |
| Recall full (frozen) | 5s | 500ms (pre-fetched) or 2s (frame-skip) |
| Related artifacts | N/A (manual search) | 0ms (returned with recall) |
| Merkle verify | N/A | <1ms (Rust sidecar) |

The goal: **cold recall feels like warm recall** because either (a) the header was pre-fetched, or (b) you only need the header anyway.

---

## Crypto Integration

All v2 features maintain the existing PQC security model:

| Operation | Crypto |
|-----------|--------|
| Summary Merkle proof | SHA3-256, verified in Rust sidecar |
| Header decompression | AES-256-GCM decrypt (per-section if encrypted) |
| Pre-fetch cache | Decrypted in-memory only, never persisted to disk |
| Activation graph | Stored encrypted (contains artifact relationships) |
| Reconsolidation | Re-encrypted after keyword/embedding update |
| Root seal | HMAC-SHA3-256 with ML-KEM-768 derived key |

---

## The Neuroscience Parallel

| Brain Process | Engram v2 Implementation | Latency |
|--------------|-------------------------|---------|
| **Cue → hippocampal pattern match** | Semantic search + HNSW | 5ms |
| **Pattern completion (gist)** | Merkle-verified summary return | 5ms |
| **Detail filling (cortical reactivation)** | Progressive section decompression | 20-200ms |
| **Spreading activation** | Activation graph auto-returns related | 0ms (pre-computed) |
| **Priming** | SessionStart pre-fetch of cold/frozen headers | Background |
| **Reconsolidation** | Re-index with current context on recall | Background |
| **Forgetting (interference)** | Low-importance artifacts demote faster | On `engram run` |
| **Sleep consolidation** | Nightly `engram run` + graph rebuild | Cron |

This isn't a metaphor. It's the same algorithm, implemented on different hardware.
