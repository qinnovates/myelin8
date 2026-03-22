# Myelin8 Data Contract

What myelin8 accepts, rejects, and guarantees.

---

## Accepted Input

| Property | Accepted | Rejected |
|---|---|---|
| **Encoding** | Valid UTF-8 | Invalid UTF-8 (reject at ingest with error) |
| **Content** | Any valid UTF-8 string including: code, markdown, JSON, JSONL, emoji, CJK, Arabic, mathematical symbols, ANSI escape codes, control characters (except null) | Null bytes (0x00) — reject at ingest |
| **File size** | 50 bytes to 10MB | <50 bytes (skip, too small), >10MB (reject with error) |
| **File types** | `.md`, `.txt`, `.jsonl`, `.json`, `.log` | Binary files (detected by null byte scan or extension: `.png`, `.jpg`, `.pdf`, `.bin`, `.exe`, `.wasm`) |
| **Line endings** | LF, CRLF, CR — all preserved as-is | None rejected |
| **Nesting** | Any JSON nesting depth | None rejected (Parquet handles arbitrary strings) |
| **Duplicates** | Detected by SHA-256 content hash, skipped silently | Not an error — dedup is expected behavior |
| **Near-duplicates** | Detected by SimHash, ingested with flag | Not rejected — flagged in metadata |

## Guarantees

### Fidelity

**The content you put in is the content you get out.** Byte-for-byte identical. Verified by SHA-256 computed on the original source file at ingest, stored alongside the content in every tier, and recomputed on every recall.

If `SHA-256(recalled_content) != stored_content_hash`, the system reports integrity failure. It does not silently return corrupted data.

### Metadata Preservation

Every artifact retains through all tier transitions:

| Field | Type | Guarantee |
|---|---|---|
| `content_hash` | SHA-256 hex string | Computed once on original. Never changes. |
| `artifact_id` | First 16 chars of content_hash | Stable identifier across all operations. |
| `created_date` | YYYY-MM-DD string | Set from file mtime at first ingest. Never changes. |
| `last_accessed` | Unix timestamp | Updated on search hit or recall. |
| `significance` | f32, 0.0–1.0 | Computed at ingest. Modified by pin/boost/decay. |
| `source_label` | String | Set by user via `myelin8 add --label`. Never changes. |
| `source_path` | String | Original file path. Never changes (even if source file moves). |
| `summary` | String | Extracted at ingest. Never changes. |
| `keywords` | Vec of strings | Extracted at ingest. Never changes. |
| `original_size` | u64 | Byte count of original content. Never changes. |

### Search

- An artifact indexed in tantivy is findable by any token it contains.
- All tokens are indexed (not top-N). Stemming is applied.
- Search NEVER reads Parquet. Results come from tantivy stored fields.
- After compaction (hot → Parquet), the artifact remains in the tantivy index. Search behavior is unchanged.
- Recall from Parquet reads only the content column (column-selective).

### Compaction

- Compaction is atomic: either all eligible files compact successfully, or none do.
- Hot files are moved to `.recycled/` (not deleted) and kept for 7 days.
- WAL manifest records in-progress compaction for crash recovery.
- Compaction is idempotent: running it twice produces no duplicates.

### Rejection Behavior

When a file is rejected (invalid UTF-8, null bytes, too large, too small):
- The file is skipped with a logged warning.
- No partial ingest occurs.
- The file does not appear in the index.
- No Parquet data is written for rejected files.
- Rejection never causes a panic or process exit.

## Not Guaranteed

- **Ranking order is not guaranteed after compaction.** The same query may rank results differently before and after compaction. What IS guaranteed: if an artifact was findable before compaction, it is findable after.
- **Summary quality.** Template-based extraction (first heading + first paragraph). May miss the most relevant content in complex documents.
- **Significance accuracy.** Heuristic scoring. "Decided" in a code comment is scored the same as "decided" in a decision narrative.
- **Entity resolution.** Two artifacts mentioning "Sarah" are not automatically linked.
- **Temporal reasoning.** The system does not understand "last Tuesday" without an explicit date range filter.
