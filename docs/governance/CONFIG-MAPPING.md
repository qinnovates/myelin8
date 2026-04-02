# Basic Config Mapping: The Simplest Viable Starting Configuration

> Start here. This is the minimum viable implementation. Add complexity only when a specific problem demands it.

---

## The Minimum Viable Stack

```
your-project/
├── CLAUDE.md                    ← 5 stanzas, under 60 lines
├── .claude/
│   └── rules/
│       ├── security.md          ← Security constraints with path scoping
│       ├── conventions.md       ← Naming and structure conventions
│       └── error-handling.md   ← Error handling rules
└── memory/
    ├── semantic_memory.jsonl    ← User preferences and stable facts
    └── session_state.json       ← Current session working state
```

No layers 2-5 yet. No vector store. No context assembly pipeline. Just CLAUDE.md done right and a simple memory file. This is better than 80% of what's currently in production.

---

## Minimal CLAUDE.md

```markdown
# Your Project Name

## Stack
[2-3 lines: language, framework, key dependencies, versions]

## Conventions
[3-5 lines: naming rules, file organization, one key style decision]

## Commands
[3-5 lines: build, test, lint commands — exact commands, no prose]

## Imports
@import .claude/rules/security.md
@import .claude/rules/conventions.md
@import .claude/rules/error-handling.md

## Prohibitions
NEVER hardcode secrets — use environment variables.
NEVER swallow errors with empty catch blocks.
NEVER use shell=True in subprocess calls.
NEVER build SQL with string concatenation.
NEVER return stack traces in HTTP responses.
```

That's it. Under 30 lines. Prohibitions at the bottom. Three rule file imports for selective loading.

---

## Minimal Security Rule File

```markdown
---
applies_to:
  - "src/api/**"
  - "src/auth/**"
  - "**/*token*"
  - "**/*secret*"
scope: "Activates on API and auth files"
---

# Security Rules

NEVER skip JWT signature verification.
NEVER trust the `alg: none` header.
ALWAYS verify JWT `aud` claim.
ALWAYS check authorization before loading data.
NEVER pass user input to subprocess or eval.
ALWAYS validate all input at the API boundary.
```

---

## Minimal Conventions Rule File

```markdown
---
applies_to:
  - "**/*.py"
  - "**/*.ts"
  - "**/*.swift"
scope: "Activates on source files"
---

# Conventions

[Your naming convention — 3-5 imperative rules]
[Your file organization rule — 1-2 imperative rules]
[Your async style — 1 imperative rule]
[Your key style decision — 1-2 imperative rules]
```

---

## Minimal Error Handling Rule File

```markdown
---
applies_to:
  - "**/*.py"
  - "**/*.ts"
  - "**/*.swift"
scope: "Activates on source files"
---

# Error Handling

NEVER use empty catch blocks.
NEVER swallow errors silently.
ALWAYS log: what failed, where, and what to do next.
ALWAYS use typed errors — not generic string messages.
NEVER expose stack traces or internal paths to callers.
```

---

## Minimal Semantic Memory File

Start with 5-10 entries you already know are true. Add more as the user explicitly states preferences.

```jsonl
{"id":"fact_001","namespace":"user","type":"preference","key":"output_format","value":"markdown","confidence":1.0,"source":"explicit","created_at":"2026-04-01T00:00:00Z","updated_at":"2026-04-01T00:00:00Z","expires_at":null,"sensitivity":"low"}
{"id":"fact_002","namespace":"user","type":"preference","key":"preferred_tone","value":"direct","confidence":1.0,"source":"explicit","created_at":"2026-04-01T00:00:00Z","updated_at":"2026-04-01T00:00:00Z","expires_at":null,"sensitivity":"low"}
{"id":"fact_003","namespace":"user","type":"identity","key":"primary_stack","value":"Python, FastAPI","confidence":1.0,"source":"explicit","created_at":"2026-04-01T00:00:00Z","updated_at":"2026-04-01T00:00:00Z","expires_at":null,"sensitivity":"low"}
```

**Retrieval:** At inference time, load the top 5 most recently confirmed facts. That's your starting retrieval strategy. You can add embedding-based similarity search later.

---

## Minimal PostToolUse Hook

Even a simple grep-based hook is better than nothing:

```bash
#!/usr/bin/env bash
# Minimal PostToolUse hook — catches the most common violations

TARGET="${1:-}"
[[ -z "$TARGET" || ! -f "$TARGET" ]] && exit 0

VIOLATIONS=0

# Block shell=True
if grep -qn 'shell=True' "$TARGET" 2>/dev/null; then
    echo "VIOLATION: shell=True detected in $TARGET"
    VIOLATIONS=$((VIOLATIONS + 1))
fi

# Block hardcoded API key patterns
if grep -qnE '(api_key|token|secret|password)\s*=\s*["\x27][a-zA-Z0-9_\-]{20,}' "$TARGET" 2>/dev/null; then
    echo "VIOLATION: Possible hardcoded credential in $TARGET"
    VIOLATIONS=$((VIOLATIONS + 1))
fi

# Block SQL string concatenation
if grep -qnE '(SELECT|INSERT|UPDATE|DELETE).*\+' "$TARGET" 2>/dev/null; then
    echo "VIOLATION: Possible SQL string concatenation in $TARGET"
    VIOLATIONS=$((VIOLATIONS + 1))
fi

[[ $VIOLATIONS -gt 0 ]] && exit 1 || exit 0
```

Install semgrep later for more precise rules. The grep-based hook ships today and catches the most critical violations.

---

## When to Add Each Layer

| Problem You're Experiencing | Solution to Add |
|-----------------------------|----------------|
| Different behavior needed for different use cases | Add Layer 2 (App config) |
| Dev behavior differs from prod in bad ways | Add Layer 3 (Environment config) |
| User preferences scattered across sessions | Formalize Layer 4 (User config) |
| Session context bleeding between conversations | Formalize Layer 5 (Session state) |
| Memory store growing too large | Add confidence filtering + 90-day decay |
| Top-N retrieval not surfacing the right facts | Add embedding-based similarity search |
| Grep-based hook missing too many violations | Upgrade to semgrep with custom rules |

**Don't add layers preemptively.** Add them when a specific problem in production demands it. The overhead of unnecessary layers costs more than the problems they'd preemptively solve.

---

## Config Maturity Model

| Level | What You Have | Adherence | Token Cost | When to Stop Here |
|-------|--------------|-----------|------------|-------------------|
| **0** | No CLAUDE.md, no memory | ~40% | N/A | Never — too low |
| **1** | Minimal CLAUDE.md (60 lines) | ~75% | Low | OK for personal projects |
| **2** | Level 1 + domain rule files | ~87% | Very Low | OK for team projects |
| **3** | Level 2 + PostToolUse hook + structured memory | ~90% | Very Low | OK for most production |
| **4** | Level 3 + layered config (Layers 1-4) + retrieval | ~93% | Minimal | For multi-user production |
| **5** | Full 5-layer architecture + audit trail + maintenance cadence | ~95%+ | Minimal | For enterprise/regulated |

Most teams should target Level 3. If you're there, you've solved the problems that matter. Levels 4-5 address scale, multi-tenancy, and compliance — not adherence quality per se.
