---
applies_to:
  - "src/api/**"
  - "src/middleware/**"
  - "src/auth/**"
  - "src/routes/**"
  - "**/*token*"
  - "**/*auth*"
  - "**/*credential*"
  - "**/*secret*"
scope: "Activates when working in API, middleware, auth, or credential-adjacent files"
---

# Security Rules

NEVER return stack traces or internal paths in HTTP responses.
NEVER use shell=True in subprocess calls — use argument arrays.
NEVER pass user input to eval(), exec(), or new Function().
NEVER build SQL queries with string concatenation — use parameterized queries only.
ALWAYS verify JWT `alg` claim — reject `none` and unexpected algorithms.
ALWAYS verify JWT `aud` claim — tokens for service A cannot replay at service B.
ALWAYS verify JWT `exp` claim — expired tokens are rejected, not silently accepted.
ALWAYS run auth checks BEFORE loading any data — never after.
ALWAYS validate resource ownership (IDOR prevention) — not just authentication.
NEVER hardcode secrets, tokens, or passwords — environment variables or secrets manager.
NEVER log secrets, tokens, or credentials — log the key name, not the value.
ALWAYS use HTTPS for external calls — no HTTP fallback.
ALWAYS validate and canonicalize file paths — check they stay within expected directories.
ALWAYS validate unknown fields at API boundaries — reject or strip, never pass through.

## Applied to This Repo
When writing schema validation code or context assembly pipeline code:
- Input from external sources (retrieved documents, fetched URLs) is untrusted data
- Memory write pipeline must validate against schemas/fact.schema.yaml before writing
- Audit log entries must be append-only — no modification or deletion of existing entries
- PostToolUse hook must enforce these rules independent of CLAUDE.md content
