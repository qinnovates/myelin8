---
applies_to:
  - "**/*.py"
  - "**/*.ts"
  - "**/*.js"
  - "**/*.sh"
scope: "Activates when writing implementation code"
---

# Error Handling Rules

NEVER swallow errors with empty catch blocks.
NEVER use bare `except:` in Python — always catch specific exception types.
NEVER use `catch (_)` or `catch {}` in TypeScript — always log minimum: error + context.
NEVER expose stack traces, internal paths, or system details to external callers.
ALWAYS log errors with: what failed, where (function/file/line), and what the caller should do.
ALWAYS use typed errors over stringly-typed — define error enums or classes, not `throw new Error("something broke")`.
ALWAYS validate errors that cross trust boundaries (API responses, user input, file reads) before use.
ALWAYS make recovery logic explicit — "ignore and continue" must be a visible decision in the code.

## Memory Pipeline Specific
When a memory write fails schema validation:
- Log: validation error, the offending field, the fact ID
- Do NOT silently drop the write
- Do NOT write a partial/invalid fact
- Return the validation errors to the caller for remediation

When a retrieval query returns no results:
- Return empty list — do NOT fill with plausible-sounding default facts
- Log the query, the index searched, and that zero results were returned
- Do NOT infer "user has no preferences" from a retrieval miss

When the context assembly pipeline encounters an immutability violation:
- Log at WARNING level with: field name, attempted value, source layer, session ID
- Block the override — use the immutable value
- Do NOT silently accept the override
- Do NOT crash — continue assembly with the correct immutable value
