# SIEMPLE-AI: Layered Context Orchestration for AI Agents

> *If you've operated a SIEM deployment at petabyte scale, you already understand how to architect AI context management. The problems are identical: layered configuration, controlled propagation, environment-specific overrides, versioning, auditability, and minimizing configuration drift. The tooling differs. The principles don't.*

---

## The Core Insight

Every production SIEM follows the same configuration model:

```
global cluster defaults
  └── app-level defaults
        └── environment overrides (dev/stage/prod)
              └── tenant-level local overrides
                    └── user-level local
                          └── runtime state
```

Each layer inherits from the one above. Local overrides win. Precedence is deterministic. Audit logs capture every write. Sensitive fields have ACLs. Ephemeral state (session KV store) never bleeds into durable config.

**AI agents need the exact same architecture.** Instead, most teams dump everything into one CLAUDE.md file, one memory.json blob, or one system prompt string — and wonder why behavior drifts, security constraints erode, and scaling to multiple use cases becomes impossible.

This repo is the reference implementation.

---

## The Critical Distinction This Repo Teaches

**CLAUDE.md is advisory context, not enforced configuration.**

It competes for attention in the context window. It degrades as sessions grow long. A transformer reading a 400-line CLAUDE.md at turn 50 of a complex session is not giving every line equal weight. Recency and proximity to the current task dominate attention.

**PostToolUse hooks running semgrep, bandit, or custom linters are the actual security boundary.**

If your security architecture depends on the model remembering a sentence in a markdown file, you don't have a security architecture. You have a suggestion.

See [`claude-code/hooks/post-tool-use.sh`](claude-code/hooks/post-tool-use.sh) for the reference implementation.

---

## The Five-Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 5: Session / Runtime  (working memory, expires fast) │
├─────────────────────────────────────────────────────────────┤
│  Layer 4: User / Tenant      (durable preferences, profile) │
├─────────────────────────────────────────────────────────────┤
│  Layer 3: Environment        (dev/stage/prod overrides)     │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: Domain / App       (use-case defaults)            │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: Base System        (identity, safety, tool policy) │
└─────────────────────────────────────────────────────────────┘
```

Last writer wins — but only for safely overridable fields. Safety boundaries and security constraints at Layer 1 are **immutable** and cannot be overridden by any downstream layer.

---

## Repository Structure

```
SIEMPLE-AI/
├── README.md                          ← This file
├── docs/
│   ├── architecture.md                ← Full five-layer system design with SIEM analogy
│   ├── claude-md-guide.md             ← How to write effective CLAUDE.md files
│   ├── memory-architecture.md         ← Memory types, schemas, retrieval patterns
│   ├── precedence-resolution.md       ← How layered config resolves at runtime
│   ├── maintenance-guide.md           ← Quarterly audit process, drift detection
│   ├── anti-patterns.md               ← What NOT to do and why
│   ├── stanza-design.md               ← Transformer attention mechanics + token savings
│   ├── ab-testing-adherence.md        ← Measuring what actually works (A/B methodology)
│   ├── siem-layered-config.md         ← SIEM layered config pattern (the source of this arch)
│   ├── best-practices-checklist.md    ← Tiered checklist for setup and quarterly audit
│   └── basic-config-mapping.md        ← Minimum viable starting configuration
├── schemas/
│   ├── fact.schema.yaml               ← Schema for semantic memory facts
│   ├── episode.schema.yaml            ← Schema for episodic memory events
│   ├── session.schema.yaml            ← Schema for session state
│   └── memory-write-policy.yaml       ← Rules governing what becomes durable memory
├── templates/
│   ├── system/
│   │   ├── defaults.yaml              ← Base system config (Layer 1)
│   │   ├── safety.yaml                ← Immutable safety boundaries
│   │   └── tools.yaml                 ← Tool policies
│   ├── apps/
│   │   ├── research/
│   │   │   ├── default.yaml           ← Research copilot app config
│   │   │   └── prompts/
│   │   │       └── summarize.md       ← Research summarization procedure
│   │   └── soc/
│   │       ├── default.yaml           ← SOC copilot app config
│   │       └── prompts/
│   │           └── triage.md          ← Alert triage procedure
│   ├── environments/
│   │   ├── dev.yaml                   ← Development environment config
│   │   ├── stage.yaml                 ← Staging environment config
│   │   └── prod.yaml                  ← Production environment config
│   ├── users/
│   │   └── example-user/
│   │       ├── preferences.yaml       ← User preference overrides (Layer 4)
│   │       ├── semantic_memory.jsonl  ← Example semantic memory store
│   │       └── profile.yaml          ← User identity and app memberships
│   └── runtime/
│       └── sessions/
│           └── example-session.json   ← Example session state (Layer 5)
├── claude-code/
│   ├── CLAUDE.md                      ← Reference CLAUDE.md (under 60 lines)
│   ├── rules/
│   │   ├── security.md                ← Path-scoped security rules
│   │   ├── consistency.md             ← Schema/config consistency rules
│   │   ├── error-handling.md          ← Error handling rules
│   │   ├── testing.md                 ← Testing requirements
│   │   └── api-design.md              ← API design rules
│   ├── commands/
│   │   ├── start-feature.md           ← Feature start workflow
│   │   ├── slop-check.md              ← Quality gate checklist
│   │   ├── build-order.md             ← Build phase sequence
│   │   └── adr.md                     ← Architecture Decision Record template
│   ├── agents/
│   │   ├── architect.md               ← Adversarial design reviewer
│   │   └── reviewer.md                ← Code/config quality reviewer
│   └── hooks/
│       └── post-tool-use.sh           ← THE actual security boundary (semgrep + bandit)
├── examples/
│   ├── context-bundle.json            ← Resolved context at inference time
│   ├── precedence-resolution.md       ← Worked config resolution example
│   └── memory-lifecycle.md            ← Memory creation → retrieval → decay → expiry
├── audit/
│   ├── config_changes.jsonl           ← Example config change audit log
│   └── memory_writes.jsonl            ← Example memory write audit log
└── LICENSE
```

---

## Documentation Map

| I want to... | Read this |
|-------------|-----------|
| Understand the full architecture | [`docs/architecture.md`](docs/architecture.md) |
| Write a better CLAUDE.md today | [`docs/claude-md-guide.md`](docs/claude-md-guide.md) + [`docs/stanza-design.md`](docs/stanza-design.md) |
| Understand the SIEM connection | [`docs/siem-layered-config.md`](docs/siem-layered-config.md) |
| Get started fast | [`docs/basic-config-mapping.md`](docs/basic-config-mapping.md) |
| Set up memory properly | [`docs/memory-architecture.md`](docs/memory-architecture.md) |
| Understand how layers merge | [`docs/precedence-resolution.md`](docs/precedence-resolution.md) + [`examples/precedence-resolution.md`](examples/precedence-resolution.md) |
| Measure adherence | [`docs/ab-testing-adherence.md`](docs/ab-testing-adherence.md) |
| Avoid common mistakes | [`docs/anti-patterns.md`](docs/anti-patterns.md) |
| Run a quarterly audit | [`docs/maintenance-guide.md`](docs/maintenance-guide.md) + [`docs/best-practices-checklist.md`](docs/best-practices-checklist.md) |
| See a working example | [`examples/context-bundle.json`](examples/context-bundle.json) + [`examples/memory-lifecycle.md`](examples/memory-lifecycle.md) |

---

## Quick Start

### For Claude Code users

1. Copy [`claude-code/CLAUDE.md`](claude-code/CLAUDE.md) to your project root
2. Copy [`claude-code/rules/`](claude-code/rules/) to `.claude/rules/`
3. Install the PostToolUse hook from [`claude-code/hooks/`](claude-code/hooks/)
4. Run the [best practices checklist](docs/best-practices-checklist.md) Tier 1 items
5. Add your semantic memory file based on [`templates/users/example-user/`](templates/users/example-user/)

### For AI platform builders

1. Read [`docs/architecture.md`](docs/architecture.md) — the full system design
2. Adopt the schemas in [`schemas/`](schemas/) for your memory stores
3. Use the templates in [`templates/`](templates/) as your config layer starting points
4. Implement [`docs/precedence-resolution.md`](docs/precedence-resolution.md) in your context assembly pipeline

---

## Why SIEM Operators Have an Advantage Here

You've already solved:
- **Layered precedence** — you know why the `[default]` stanza matters and why local overrides win
- **Config drift** — you've chased stanzas overriding each other in unexpected ways
- **Separation of concerns** — different config files serve different purposes; you don't mix them
- **Audit trails** — every config change logged, who changed what, when
- **Environment promotion** — you don't deploy the same resource limits to dev and prod
- **Knowledge object governance** — old objects accumulate, cause problems; you've built processes to manage them

The gap is recognizing that AI "memory" is not a flat file — it's a KV store with confidence scores, TTLs, relevance ranking, and contradiction detection. It's the SIEM's KV Store crossed with a knowledge object governance system. Once you see it that way, the architecture is obvious.

The failures you've already experienced (config drift, knowledge object entropy, dev/prod divergence, the monolith problem) map directly to the AI failure modes this architecture prevents.

---

## The Architecture Phrase

> **Layered context orchestration with memory retrieval and deterministic precedence.**

---

## License

MIT. Use it, fork it, adapt it.
