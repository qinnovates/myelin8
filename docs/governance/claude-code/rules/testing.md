---
applies_to:
  - "**/*test*"
  - "**/*spec*"
  - "tests/**"
scope: "Activates when writing or editing test files"
---

# Testing Rules

WRITE tests before implementation for memory pipeline components.
NEVER hit real vector indices in unit tests — mock retrieval results.
NEVER write to the real semantic memory store in tests — use a test fixture store.
ALWAYS test the precedence resolution algorithm with fixture layer stacks.
ALWAYS test immutability violations — verify the pipeline blocks the override.
ALWAYS test conflict detection — verify flagging behavior, not just the winning value.
ALWAYS test the PostToolUse hook with known violation patterns — verify non-zero exit code.

## Required Test Coverage for Core Components

### Context Assembly Pipeline
- [ ] Clean merge across all 5 layers
- [ ] Immutability violation is blocked and logged
- [ ] Additive merge (arrays are unioned, not replaced)
- [ ] Session-only fields do not persist after session_end
- [ ] Retrieval budget is respected (max-N results)

### Memory Write Gate
- [ ] Explicit preference writes succeed with audit log entry
- [ ] Single-session inference is held in pending_memory_writes, not committed
- [ ] PII patterns in fact values are rejected
- [ ] Credential patterns in fact values are rejected
- [ ] Confidence > 0.8 on inferred facts is capped at 0.8
- [ ] Contradiction detection flags conflicting facts

### PostToolUse Hook
- [ ] `shell=True` in Python subprocess is detected and blocked
- [ ] Hardcoded credential patterns are detected
- [ ] SQL string concatenation is detected
- [ ] Clean code produces zero violations (false-positive check)
