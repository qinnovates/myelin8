# Stanza Design: How to Structure CLAUDE.md for Maximum Adherence

> This document explains the transformer attention mechanics that determine whether your CLAUDE.md rules are actually followed — and how the five-stanza design exploits those mechanics.

---

## Why Structure Matters: Transformer Attention Is Not Uniform

A transformer reading a 400-line CLAUDE.md does not give every line equal weight. This is not a bug or a limitation you can configure away. It's a fundamental property of how attention works.

Two attention biases are relevant here:

### 1. Recency Bias (Primacy-Recency Effect in Context)

Content closer to the current turn receives higher attention weight. In a long session with a large CLAUDE.md, the rules you wrote at line 10 are competing with your task description at line 390 — and the task description is winning.

**Implication:** Put your most critical constraints at the **bottom** of CLAUDE.md, not the top. The prohibitions stanza goes last. This is counterintuitive — you'd think the most important things go first. But you're not writing documentation. You're writing a configuration file that gets processed by an attention mechanism that weights recency.

### 2. Task-Proximity Bias

Content semantically relevant to the current task receives higher attention weight. A rule about SQL injection gets more attention when you're working on a database query than when you're editing a README.

**Implication:** Domain-specific rules (auth, API design, testing) belong in scoped rule files — loaded via `@import` only when the current task touches their domain. A rule about JWT validation sitting in the root CLAUDE.md is wasting context budget on every task, not just auth-adjacent ones.

---

## The Token Cost of Bloat

Every line of CLAUDE.md that isn't relevant to the current task is:
1. A token consumed from the context budget
2. Noise that slightly dilutes the signal of every other line

The relationship between context length and adherence is not linear — it degrades faster than you expect as files grow.

**Rough empirical estimates** (based on observed model behavior, not formal study):

| CLAUDE.md Size | Expected Adherence on Specific Rule |
|----------------|-------------------------------------|
| Under 60 lines | ~90-95% |
| 60-150 lines | ~80-90% |
| 150-300 lines | ~65-80% |
| 300-500 lines | ~50-70% |
| Over 500 lines | ~40-60% and degrading |

These are approximations. Actual adherence depends on: rule specificity (imperative vs descriptive), rule position (recency), session length (longer sessions compress early context), and task complexity (more complex tasks leave less attention for constraints).

The point is: **a 400-line CLAUDE.md is not twice as good as a 200-line one. It may be worse.**

---

## The Context Token Math

Here's the actual cost of different approaches:

```
Scenario A: One god CLAUDE.md file (common anti-pattern)
────────────────────────────────────────────────────────
CLAUDE.md: 350 lines, ~4,200 tokens
Injected: every turn, regardless of task
Relevant content per turn: ~30% (if you're lucky)
Wasted tokens per turn: ~2,940

Scenario B: Five-stanza with domain file splitting (this architecture)
──────────────────────────────────────────────────────────────────────
Root CLAUDE.md: 55 lines, ~660 tokens (always injected)
security.md: 40 lines, ~480 tokens (injected only on auth/API files)
consistency.md: 30 lines, ~360 tokens (injected only on new file creation)
error-handling.md: 30 lines, ~360 tokens (injected on implementation files)
testing.md: 30 lines, ~360 tokens (injected on test files)
api-design.md: 35 lines, ~420 tokens (injected on API files)

Typical turn (editing a schema file):
  Root CLAUDE.md: 660 tokens (always)
  consistency.md: 360 tokens (schema files trigger this)
  Total: 1,020 tokens

Auth-heavy turn (editing auth middleware):
  Root CLAUDE.md: 660 tokens
  security.md: 480 tokens
  api-design.md: 420 tokens
  Total: 1,560 tokens

Tokens saved vs Scenario A: 2,640-3,180 tokens per turn
```

Over 100 turns in a session: **264,000–318,000 tokens saved** that are now available for actual working context (code, outputs, turn history).

That's not a small optimization. That's the difference between being able to keep 20 extra turns of history in context vs. being forced to compress.

---

## The Five-Stanza Skeleton

```
┌──────────────────────────────────────────────┐
│  STANZA 1: Stack                             │  ← Model needs this immediately
│  What technologies, versions, dependencies  │    to understand any task
├──────────────────────────────────────────────┤
│  STANZA 2: Conventions                       │  ← Shapes how code is written
│  Naming, organization, style                │    throughout the session
├──────────────────────────────────────────────┤
│  STANZA 3: Commands                          │  ← Operational knowledge
│  Build, test, lint, run                     │    needed at BUILD phase
├──────────────────────────────────────────────┤
│  STANZA 4: Imports                           │  ← On-demand domain rules
│  @import references to domain files         │    loaded only when needed
├──────────────────────────────────────────────┤
│  STANZA 5: Prohibitions          ← LAST ──  │  ← Recency-weighted for max
│  NEVER / ALWAYS hard constraints           │    attention at constraint time
└──────────────────────────────────────────────┘
```

### Why This Order

**Stanzas 1-3 come first** because the model needs project context (what stack is this? how do I run the tests?) from turn 1. This context informs every subsequent task.

**Stanza 4 (imports) is in the middle** to serve as the transition point between static context (stanzas 1-3) and the prohibitions. The @import mechanism defers loading until needed.

**Stanza 5 (prohibitions) is LAST** because:
- Recency bias means it gets higher attention weight at every turn
- It's closer in token distance to the actual user message
- In a long session, it's the section most likely to survive context compression in working memory

---

## Imperative vs Descriptive Framing: Context Token Efficiency

Beyond position, the *form* of rules affects both adherence and token efficiency.

**Descriptive framing** (weak and verbose):
```markdown
The team has historically found that using shell=True in subprocess calls creates
security vulnerabilities by allowing shell injection attacks. We've seen this cause
production incidents in the past, so the project generally avoids this pattern.
```
Tokens: ~45 | Adherence: low (hedged, past-tense, no clear directive)

**Imperative framing** (strong and concise):
```markdown
NEVER use shell=True in subprocess calls.
```
Tokens: 10 | Adherence: high (clear, present-tense directive)

**Ratio: 4.5x more tokens for lower adherence.** Descriptive framing is strictly worse in every measurable dimension.

The transformer's pattern-matching responds to the structural signature of a constraint. "NEVER X" pattern-matches to "this is a hard constraint." A paragraph about past incidents pattern-matches to "this is context/history" and gets weighted accordingly.

---

## The Selective Loading Test

To verify your @import mechanism is actually selective:

```bash
# Check what gets loaded on a schema file edit
claude --show-context-sources "Edit schemas/fact.schema.yaml to add a tags field"
# Should show: CLAUDE.md + consistency.md (schema files) + maybe api-design.md
# Should NOT show: security.md (no auth/API patterns in the task)
# Should NOT show: testing.md (no test file involved)
```

If you see all rule files loaded on every task, your path scoping is not working. Check the `applies_to:` frontmatter in each rule file.

---

## The Adherence Test

Run this quarterly. Track the results over time.

```bash
# Test 1: Core constraint recall
claude "Summarize my CLAUDE.md in 5 points covering the most important constraints"
# Verify: all prohibitions are represented accurately

# Test 2: Rule framing test — does imperative framing outperform descriptive?
# Take a rule in imperative form, rewrite it descriptively, test adherence to each
# (Run 10 tasks designed to trigger each version, score compliance manually)

# Test 3: Recency test — are prohibitions at bottom more adhered to than if at top?
# Create two versions of CLAUDE.md (prohibitions at top, prohibitions at bottom)
# Run 20 tasks targeting the prohibitions, score compliance
# Expect: bottom > top by 5-15 percentage points
```

---

## YAML Example: Before and After

### Before (200-line monolith, excerpt)

```markdown
# CLAUDE.md

## Important Rules
We avoid using shell=True because of past security incidents. The team has
found that subprocess calls with shell=True can lead to injection attacks...
[40 more lines about why...]

## Error Handling Philosophy
Good error handling is important. We try to avoid swallowing errors because
that makes debugging harder. When we write catch blocks we like to include
at least some logging...
[30 more lines of rationale...]

## Security Considerations
Security is a top priority for the team. We follow OWASP guidelines and
try to think about security at every stage of development...
[50 more lines of general security principles...]
```

**Problems:**
- ~350 tokens of context to communicate ~15 tokens of actual rules
- Buried in rationale and history
- Descriptive framing, no clear directives
- Everything in one file, loaded every turn

### After (five-stanza, 55 lines total)

```markdown
# Project Name

## Stack
Python 3.12, FastAPI, PostgreSQL 16, Redis

## Conventions
Repository pattern for data access. Feature-grouped file structure.
async/await throughout — never .then() chains.

## Commands
make test    # pytest + coverage
make lint    # ruff + mypy
make dev     # uvicorn --reload

## Imports
@import .claude/rules/security.md
@import .claude/rules/error-handling.md

## Prohibitions
NEVER use shell=True in subprocess calls.
NEVER swallow errors with empty catch blocks.
NEVER build SQL with string concatenation.
NEVER hardcode secrets — use environment variables.
NEVER return stack traces in HTTP responses.
```

**Result:** 60 lines, ~720 tokens, clear directives, prohibitions at bottom for recency weighting, domain rules loaded on-demand.
