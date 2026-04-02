# SIEM Layered Configuration Management: The Source of This Architecture

> This document explains the enterprise SIEM configuration pattern that inspired the five-layer AI architecture. Understanding where the pattern comes from is important for understanding where it extends and where the analogy breaks.

---

## The Pattern: Hierarchical Configuration with Deterministic Precedence

Enterprise SIEM platforms that process petabyte-scale event streams share a common configuration architecture. It wasn't designed in one place — it emerged from the operational reality of running distributed log collection and analysis at scale, where:

- Hundreds of data sources need different parsing rules
- Multiple teams need to configure behavior without overwriting each other
- Changes in one app must not break another
- Production cannot have the same configuration as development
- Individual analysts may need personal overrides that don't affect the shared platform

The solution every mature SIEM platform converged on: **hierarchical, file-based configuration with deterministic merge precedence**.

---

## The Configuration File Hierarchy

SIEM platforms separate configuration into multiple specialized files, each responsible for a specific concern:

```
Collection tier:           What to collect and how
  inputs.conf             ← Data source definitions, forwarder behavior

Parsing tier:              How to parse and structure raw data
  props.conf              ← Source-type properties, timestamp parsing, charset
  transforms.conf         ← Field extraction rules, lookup table definitions
  fields.conf             ← Custom field declarations

Index tier:                How and where to store
  indexes.conf            ← Index definitions, retention, storage paths
  limits.conf             ← Performance limits, queue sizes, memory caps

Search and analysis tier:  What to do with the data
  savedsearches.conf      ← Saved searches, scheduled reports, alerts
  eventtypes.conf         ← Event type definitions for classification
  macros.conf             ← Search macro definitions
  tags.conf               ← Tag-to-event-type mappings

Correlation tier:          Cross-source analysis
  correlationsearches.conf ← ES correlation search definitions
  notables.conf           ← Notable event configurations
```

Each file has a specific scope. `props.conf` does not define alerts. `savedsearches.conf` does not define field extractions. This **separation of concerns** means that a team managing field extractions cannot accidentally break alert schedules, and vice versa.

---

## The Directory Precedence Hierarchy

For each configuration file type, there is a well-defined directory precedence order. The merge is deterministic: settings from higher-precedence directories override settings from lower-precedence directories for the same stanza key.

```
system/default/          ← Platform-shipped defaults (read-only)
  ↑ lowest precedence (for overridable settings)
  ↑ HIGHEST precedence for platform-immutable settings

system/local/            ← Global cluster overrides (ops team manages)
  ↑ overrides system/default

etc/apps/<app>/default/  ← App-shipped defaults (app developer manages)
  ↑ overrides system-level for this app's scope

etc/apps/<app>/local/    ← App local overrides (ops team customizes)
  ↑ overrides app/default

etc/users/<user>/        ← User-level personal configuration
  ↑ overrides app-level for this user's sessions
```

**Merge algorithm:** For each `[stanza]` and each `key = value` within that stanza, the value from the highest-precedence directory wins. If a key exists only in `system/default` and nowhere else, the system default applies everywhere. If `app/local` overrides the same key, `app/local` wins.

There is no ambiguity. Given the configuration files and the precedence hierarchy, the resolved value for any key is deterministic.

---

## The Immutability Analog

SIEM platforms have a capability system — certain operations require specific capabilities (`admin`, `power`, `user`). An app configuration cannot grant itself capabilities it wasn't shipped with. A `system/local` restriction can override app settings, but cannot grant capabilities below the system's allowed set.

This is the equivalent of the AI architecture's immutable Layer 1 fields:
- The app (Layer 2) cannot expand permissions beyond what the system (Layer 1) allows
- The user (Layer 4) cannot override safety boundaries set at Layer 1
- The session (Layer 5) can only add restrictions, never expand them

---

## The App Isolation Pattern

A key SIEM design principle: apps should be able to run alongside each other without conflict. Each app lives in its own directory. Each app's configuration is scoped to its own stanzas. A change to one app's `props.conf` does not affect another app's parsing.

Translated to AI architecture:
- The `research_copilot` app configuration does not affect the `soc_copilot` app
- App-layer config is scoped to sessions using that app
- Shared fields (like `safety.pii_redaction`) are managed at the system layer, not duplicated per app

---

## The Environment Promotion Pattern

SIEM deployments always have at least two environments: non-production (for testing new content, parsing rules, alert logic) and production (live). The configuration difference between environments must be minimal and well-defined.

The principle: **the same app, the same configuration hierarchy, different environment-specific overrides**. Not different codebases. Not different architectures. Just a different `limits.conf` for the production indexers and a different `inputs.conf` for which data sources are active.

In the AI architecture, this is Layer 3 (Environment). The `research_copilot` app is identical in dev and prod. The environment layer handles: which model to use, which vector index to point at, what logging verbosity to apply, what resource limits to enforce.

---

## The Distributed Collection Challenge → Retrieval Architecture

SIEM platforms at petabyte scale don't ship all data to one place and search it in bulk. They use indexing infrastructure: data is parsed and indexed at collection time so that searches operate against structured indexes rather than raw text.

When a user runs a search, they are not full-table-scanning billions of raw events. They are querying an inverted index with field-value pairs, time ranges, and pre-computed aggregations.

**This is exactly the right model for AI memory retrieval.** You are not scanning the full memory store on every turn. You are querying a similarity index with the current turn embedding and retrieving the top-N most relevant facts. The SIEM's inverted index and the AI memory's vector store serve the same architectural role.

The failure mode is also the same: if you run `index=*` (query everything) instead of `index=security sourcetype=firewall`, you get correct results but at enormous cost. If you inject all semantic memory instead of retrieving top-N, you get correct coverage but at enormous context window cost.

---

## The Knowledge Object Governance Problem

SIEM platforms have "knowledge objects": saved searches, event types, tags, macros, field aliases. These are shared, reusable configuration objects that many searches and dashboards depend on.

The governance challenge: knowledge objects accumulate over time. Nobody deletes them because "something might be using it." Old macro definitions shadow new ones. Deprecated event types stay in the system. After three years, the knowledge object layer is a mess.

AI memory has the exact same problem. Facts accumulate. Old inferences never expire. Contradictory facts coexist. After 18 months of use, the semantic memory store contains:
- Stale preferences from projects long since finished
- Inferences that were never confirmed
- Contradictory facts (both `output_length: concise` and `output_length: thorough`)
- Obsolete constraints for libraries no longer in use

The SIEM solution: periodic knowledge object audits, ownership tracking, usage metrics to identify unused objects, automated expiration for objects that haven't been used in 90 days.

The AI solution: exactly the same. That's the 90-day inferred fact decay policy. That's the quarterly audit protocol. The problem is old; the solution is the same.

---

## Where the SIEM Analogy Breaks (and What Replaces It)

| SIEM Concept | AI Equivalent | Key Difference |
|-------------|--------------|----------------|
| `conf` file key-value store | Semantic memory JSONL | AI store needs confidence scores, TTLs, contradiction detection |
| Deterministic merge | Config layer merge (Layers 1-5) | Same pattern, same algorithm |
| Capability ACLs | Immutable safety fields | Same concept, different enforcement mechanism |
| App isolation | Layer 2 app configs | Direct analog |
| KV Store (runtime lookups) | Session working memory | KV Store persists; session memory expires |
| Knowledge objects | Procedural memory + semantic memory | Knowledge objects are deterministic; AI memory has uncertainty |
| Index retention policy | Memory TTL + compaction | Similar concept; AI adds compaction/summarization step |
| Search-time field extraction | Context assembly pipeline | Both produce structured data from raw input |
| Saved searches | Procedural memory / custom commands | Direct analog for workflow procedures |
| Alert actions | PostToolUse hooks | Triggered by events, enforce rules, run outside main pipeline |

**The fundamental difference:** SIEM configuration is deterministic. A `props.conf` stanza either matches or it doesn't. AI memory is probabilistic — confidence scores, relevance rankings, recency decay, and inference quality all introduce uncertainty that has no SIEM equivalent.

**The extended analogy:** SIEM + Bayesian knowledge base + KV Store with TTL + retrieval-augmented search index. The layered config management pattern is SIEM. The memory system is something new that needs to be built on top of it.

---

## Lessons from Operating at Scale

If you've managed a SIEM deployment that processed petabytes per day, you've already experienced these failure modes:

1. **Configuration drift:** A stanza added three years ago that nobody remembers, overriding a setting in an unexpected way. → AI equivalent: stale semantic facts with no TTL
2. **Merge surprises:** A setting in `system/local` unexpectedly overriding an app's intended behavior. → AI equivalent: a Layer 1 immutability violation not being caught
3. **The monolith problem:** One massive search that does everything, impossible to optimize. → AI equivalent: one god CLAUDE.md, one memory.md blob
4. **Knowledge object entropy:** Knowledge objects nobody will delete that subtly distort every search. → AI equivalent: old, low-confidence inferred facts poisoning retrieval
5. **The dev/prod divergence incident:** A configuration that works perfectly in dev but fails in prod because of different `limits.conf`. → AI equivalent: different model behavior in dev (haiku, verbose logging) vs prod (opus, restricted logging)

The SIEM operators who have hit all five failure modes are exactly the right people to build AI context management systems. The failures are familiar. The solutions are familiar. The tooling is new.
