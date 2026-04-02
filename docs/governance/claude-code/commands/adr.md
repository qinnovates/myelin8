# /adr — Architecture Decision Record

Use this command when making a significant design decision: new dependency, schema change, layer interaction, or any decision with a reversal cost > 1 day.

## ADR Template

```markdown
# ADR-{number}: {Decision Title}

**Date:** {YYYY-MM-DD}
**Status:** PROPOSED | ACCEPTED | DEPRECATED | SUPERSEDED BY ADR-{number}

## Context
What is the situation that requires a decision?
What forces are at play (technical constraints, scale requirements, security requirements)?

## Decision Drivers
- Driver 1 (e.g., "must support per-user confidence thresholds")
- Driver 2
- Driver 3

## Options Considered

### Option A: {Name}
**Pros:**
- Pro 1
- Pro 2

**Cons:**
- Con 1
- Con 2

**Risk:** {low|medium|high} — {one-line reason}

### Option B: {Name}
[Same structure]

## Decision
**Chosen:** Option {X}

**Rationale:** (2-3 sentences connecting the chosen option to the decision drivers)

## Consequences
**Positive:**
- Consequence 1

**Negative / Tradeoffs:**
- Tradeoff 1

**Reversal Cost:** {low|medium|high} — {what would it take to undo this?}

## Open Questions
- Question 1 (to be resolved by {date|condition})
```

## When to Generate an ADR

Criteria — generate an ADR if any of these are true:
- New dependency being added
- Schema change that affects existing stored data
- Change to the context assembly algorithm
- Change to immutability or override behavior
- New layer interaction pattern
- Reversal cost > 1 day of engineering work
- More than one reasonable approach exists

## Filing
Save as `docs/adr/ADR-{NNN}-{kebab-title}.md` where NNN is zero-padded (ADR-001, ADR-002, etc.).
Link from `docs/architecture.md` under "Architecture Decisions."
