# How to Write Effective CLAUDE.md Files

> The most common mistake: treating CLAUDE.md like documentation. It isn't. It's a configuration file that competes for transformer attention in a finite context window. Every line you add dilutes every other line.

---

## The Attention Budget Problem

A transformer reading your CLAUDE.md doesn't give every line equal weight. Attention is:

- **Recency-weighted**: content near the current turn gets more weight than content 400 lines back
- **Task-proximity weighted**: content relevant to the current task gets more weight than general rules
- **Dilution-affected**: a 400-line CLAUDE.md means each rule gets roughly 1/400th of the budget

This has concrete implications for how you write and structure the file.

---

## The Five-Stanza Skeleton

Every CLAUDE.md should follow this structure. Order is not arbitrary — it reflects what the model needs to know first vs. what benefits from recency weighting.

```markdown
# Project Name

## Stack
[Technology stack, key dependencies, language versions]

## Conventions
[Naming, file organization, style rules — imperative framing]

## Commands
[How to build, test, lint, run — exact commands]

## Imports
[Optional: @import references to domain-specific rule files]

## Prohibitions
[NEVER/ALWAYS statements — put these LAST for recency attention weighting]
```

The prohibitions stanza goes at the **bottom** deliberately. Transformers weight recent context more heavily. Putting your hard constraints at the bottom of the file means they're closer to the current turn in attention terms. This is the same reason you put the most important information at the bottom of a `limits.conf` stanza when you need it to actually apply.

---

## Size Constraints

| Threshold | Status |
|-----------|--------|
| Under 60 lines | Ideal |
| 60-200 lines | Acceptable |
| Over 200 lines | Split into domain files |
| Over 300 lines | Adherence degrades significantly |

**Five 30-line domain files outperform one 150-line file in adherence rate.** Domain rule files loaded via `@import` are retrieved into context only when relevant — they don't occupy baseline context budget.

Test your CLAUDE.md with: `claude "Summarize my CLAUDE.md in 5 points"`. If the model can't accurately reproduce all critical constraints, the file is either too long, too vague, or structured poorly.

---

## Imperative Framing

Descriptive framing is weak. Imperative framing is strong.

| Weak (descriptive) | Strong (imperative) |
|-------------------|---------------------|
| "The project avoids shell injection" | `NEVER use shell=True in subprocess calls` |
| "We prefer async/await" | `ALWAYS use async/await. Never use .then() chains` |
| "Security is important" | `NEVER hardcode secrets. Use environment variables` |
| "Tests should be written" | `WRITE tests before implementation. No exceptions` |

The imperative form is unambiguous, short, and pattern-matches better with how models internalize rules. It also makes violations easier to detect with `grep` or a linter.

---

## Domain Rule Files with Path Scoping

When a rule only applies to certain files or directories, it doesn't belong in the root CLAUDE.md. It belongs in a scoped domain rule file.

```markdown
---
# .claude/rules/security.md
applies_to:
  - "src/api/**"
  - "src/middleware/**"
  - "src/auth/**"
scope: "These rules activate when working in API, middleware, or auth directories"
---

# Security Rules

NEVER return stack traces in HTTP responses.
ALWAYS validate JWT `aud` claim before trusting token payload.
NEVER pass user input to subprocess or eval.
ALWAYS use parameterized queries. String interpolation in SQL is a build blocker.
```

The YAML frontmatter is a contract: these rules are injected when the working file matches the glob patterns. They don't occupy context budget when you're working on something unrelated — writing a README, updating a schema, or fixing a test.

This is the CLAUDE.md equivalent of Splunk's `[source::...]/[sourcetype::...]` stanza scoping. Rules apply where they're relevant.

---

## The Reference CLAUDE.md

See [`../claude-code/CLAUDE.md`](../claude-code/CLAUDE.md) for the complete reference implementation. Key properties:

- Under 60 lines
- Five-stanza skeleton
- Prohibitions at the bottom
- @imports for domain rules
- Imperative framing throughout
- No documentation prose — only directives

---

## Auto-Memory Scope

CLAUDE.md is for **workflow habits and project conventions**, not security constraints.

Auto-memory (the model learning your preferences over sessions) should capture:
- Output format preferences
- Tone preferences
- Recurring workflow patterns
- Stack-specific conventions

Auto-memory should **NOT** be the mechanism for security rules. If you're relying on the model remembering "don't use eval()" from a previous session's memory file, you don't have a security control. Security constraints belong in:
1. CLAUDE.md prohibitions stanza (advisory, recency-weighted)
2. PostToolUse hooks (enforced, outside context window)

---

## Quarterly Audit Protocol

CLAUDE.md files drift. Rules added three months ago may:
- Contradict newer rules added since
- Reference stale dependency versions
- Duplicate rules already covered in domain files
- Have been superseded by new tooling (a linter now catches what the rule was preventing)

Quarterly audit checklist:
1. Run `grep -n "NEVER\|ALWAYS\|MUST\|SHALL" .claude/rules/*.md CLAUDE.md | sort` — check for contradictions
2. Check every version pin mentioned: `python>=3.11`, `node>=20` — are these still current?
3. For every `@import` reference — does the file still exist?
4. Count total lines across CLAUDE.md + all rules files. If over 500 cumulative lines, something needs to be cut.
5. Run the adherence test: `claude "Summarize my CLAUDE.md in 5 points"` — can the model reproduce all critical constraints?

---

## Common Mistakes

### Mistake 1: Documentation prose in CLAUDE.md

```markdown
# Bad
The project follows the repository pattern for data access. We've found that separating
business logic from persistence concerns makes testing easier and allows us to swap
databases in the future. The repository interface is defined in src/repositories/...
```

```markdown
# Good
Data access: use repository pattern. Interfaces in src/repositories/. Never call DB from service layer.
```

### Mistake 2: Rules without teeth

```markdown
# Bad (descriptive, unverifiable)
The team cares about security and follows best practices for authentication.

# Good (imperative, grep-verifiable)
NEVER skip JWT signature verification. NEVER trust `alg: none`. ALWAYS verify `aud` claim.
```

### Mistake 3: One god file

If your CLAUDE.md has sections for API design, testing philosophy, database patterns, security rules, deployment procedures, and coding style — split it. Each of those is a domain file. The root CLAUDE.md is an index.

### Mistake 4: Putting prohibitions at the top

```markdown
# Bad: prohibitions at top, conventions at bottom
## Prohibitions
NEVER use eval()
NEVER hardcode secrets
...

## Stack
Python 3.12, FastAPI, PostgreSQL
...
```

Flip it. Put stack and conventions first (the model needs that context for the whole session), put prohibitions last (recency weighting puts them closer to the current turn).

### Mistake 5: Treating CLAUDE.md as the security boundary

CLAUDE.md is **advisory**. Install the PostToolUse hook. Run semgrep. Trust the hook, not the markdown.
