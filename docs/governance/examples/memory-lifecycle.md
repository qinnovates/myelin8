# Memory Lifecycle — Worked Example

> A complete walkthrough of how a fact moves from a user utterance through the write gate, into semantic memory, persists across sessions, decays, and eventually expires.

---

## The Fact: User States a Communication Preference

**Session:** `sess_example_001`
**Turn 4 of the session**
**User says:** "I always want direct answers — don't soften things or hedge unnecessarily"

---

## Phase 1: Detection

The context assembly pipeline processes the turn and the model identifies a preference statement.

**Classifier output:**
```json
{
  "type": "preference_statement",
  "confidence": 0.98,
  "extracted_preference": {
    "key": "preferred_tone",
    "value": "direct",
    "negations": ["don't soften", "don't hedge"]
  },
  "source_trigger": "I always want direct answers"
}
```

**Write policy check:**
- Trigger type: `explicit_preference` ✓
- `auto_write: true` ✓
- `requires_confirmation: false` ✓
- Confidence: `1.0` (explicit statement)

---

## Phase 2: Conflict Detection

Before writing, the pipeline checks for existing facts with the same or semantically similar key.

```
Search: namespace=user, key≈"preferred_tone"

Results:
  - No existing fact found for key "preferred_tone"
  - Embedding similarity check: "response_style" key has cosine similarity 0.71 to "preferred_tone"
    → Below 0.85 threshold for automatic conflict flagging
    → Proceed with write
```

---

## Phase 3: Fact Construction

```json
{
  "id": "fact_tone_pref",
  "namespace": "user",
  "type": "preference",
  "key": "preferred_tone",
  "value": "direct",
  "confidence": 1.0,
  "source": "explicit",
  "created_at": "2026-01-15T09:00:00Z",
  "updated_at": "2026-01-15T09:00:00Z",
  "last_confirmed_at": "2026-01-15T09:00:00Z",
  "expires_at": null,
  "sensitivity": "low",
  "tags": ["communication", "style"],
  "source_session_id": "sess_example_001"
}
```

---

## Phase 4: Write and Audit

**Write:** Appended to `semantic_memory.jsonl` for `user/example-user/`.

**Audit log entry:**
```json
{"ts":"2026-01-15T09:00:00Z","event":"memory_write","layer":"user","user_id":"example-user","fact_id":"fact_tone_pref","key":"preferred_tone","value":"direct","confidence":1.0,"source":"explicit","session_id":"sess_example_001","trigger_utterance":"I always want direct answers"}
```

---

## Phase 5: Retrieval in Future Sessions

Three weeks later, in a new session:

**Session:** `sess_example_042`
**Current turn:** "Explain the tradeoffs between PostgreSQL and MongoDB for this use case"

**Retrieval query embedding:** [vector representing the current task]
**Similarity search results:**
```
fact_tone_pref           relevance: 0.72  ← retrieved (above 0.5 threshold)
fact_output_format       relevance: 0.70  ← retrieved
fact_siem_background     relevance: 0.65  ← retrieved
fact_current_focus       relevance: 0.61  ← retrieved
fact_confidence_scores   relevance: 0.58  ← retrieved
fact_avoid_tables        relevance: 0.44  ← NOT retrieved (below threshold)
```

`fact_tone_pref` is injected into the context. The model gives a direct, non-hedged comparison of PostgreSQL vs MongoDB.

---

## Phase 6: Confirmation Keeps the Fact Alive

In session `sess_example_042`, the user says: "Good, that's exactly the kind of direct answer I want."

**Confirmation detected:**
- This is indirect confirmation of the `preferred_tone: direct` preference
- `last_confirmed_at` is updated to `2026-02-05T14:00:00Z`
- Confidence stays at 1.0 (was already explicit)

**Audit log:**
```json
{"ts":"2026-02-05T14:00:00Z","event":"memory_confirm","fact_id":"fact_tone_pref","session_id":"sess_example_042","confirmation_type":"indirect"}
```

---

## Phase 7: What Happens to an Inferred Fact (Contrast)

For comparison — an inferred fact with different lifecycle behavior.

**Session:** `sess_example_015`
The user reformats a table output into bullet points without commenting.

**Classifier:**
```json
{
  "type": "behavioral_signal",
  "confidence": 0.45,
  "signal": "user_reformatted_table_to_bullets",
  "proposed_preference": {"key": "tables_preference", "value": false}
}
```

**Write policy check:**
- Trigger type: `inferred_single_session`
- `auto_write: false` ← Single-session inferences do NOT auto-write
- Goes to `pending_memory_writes` in session state

**At session close:** The pending write is presented to the user:
> "I noticed you reformatted the table output into bullets. Should I remember to avoid tables for you in the future?"

User: "Yes, please remember that."

Now it becomes an explicit confirmation → written with `confidence: 1.0, source: explicit`.

If the user had said nothing → the pending write is **discarded** at session close. It never reaches semantic memory from a single session.

---

## Phase 8: Inferred Fact Decay (After 90 Days Without Confirmation)

Scenario: The inferred fact `fact_avoid_tables` (confidence 0.85, source: inferred) hasn't been confirmed in 95 days.

**Weekly decay job runs:**
```python
days_since_confirmation = 95
confidence_before = 0.85

# 90-day inferred decay policy
if source == "inferred" and days_since_confirmation > 90:
    new_confidence = confidence_before * 0.5
    # 0.85 * 0.5 = 0.425

if new_confidence < 0.3:
    # Delete the fact
    semantic_memory.delete("fact_avoid_tables")
else:
    # Update confidence, flag for review
    semantic_memory.update("fact_avoid_tables", confidence=0.425)
```

**Result:** Confidence drops from 0.85 to 0.425. Fact is still in the store but will be below retrieval threshold in most queries now. At the next decay cycle (another 90 days), `0.425 * 0.5 = 0.21` — below 0.3 threshold, deleted.

**Audit log:**
```json
{"ts":"2026-06-15T03:00:00Z","event":"memory_confidence_decayed","fact_id":"fact_avoid_tables","old_confidence":0.85,"new_confidence":0.425,"reason":"90_day_inferred_decay","days_since_confirmation":95}
```

---

## Phase 9: Fact Contradiction

New session. User says: "Actually, use tables when comparing more than 3 options — they're clearer."

**Conflict detection:**
```
New proposed fact: {"key": "tables_preference", "value": "use_for_comparisons"}
Existing fact: {"key": "tables_preference", "value": false, "confidence": 0.425}
```

Semantic conflict detected. Trigger: `same_key_same_namespace`.

**Resolution:**
- New write has `source: explicit` → beats existing `source: inferred` automatically
- Old fact is archived (not deleted — audit trail preserved)
- New fact written with confidence 1.0

**Audit log:**
```json
{"ts":"2026-07-01T11:00:00Z","event":"memory_conflict_resolved","old_fact_id":"fact_avoid_tables","new_fact_id":"fact_tables_for_comparisons","resolution":"explicit_beats_inferred","old_value":false,"new_value":"use_for_comparisons"}
```
