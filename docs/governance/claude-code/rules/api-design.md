---
applies_to:
  - "src/api/**"
  - "src/pipeline/**"
  - "**/*interface*"
  - "**/*schema*"
scope: "Activates when designing APIs or pipeline interfaces"
---

# API Design Rules

ALWAYS validate input at the boundary — not deep in business logic.
ALWAYS reject unknown fields — `additionalProperties: false` in JSON Schema.
ALWAYS use typed return values — no `any`, no untyped dicts.
NEVER return internal implementation details in API responses.
NEVER use HTTP GET for operations with side effects.
ALWAYS version APIs from day one — `/v1/` prefix.

## Context Assembly Pipeline API

The context assembly pipeline has one entry point:

```
assemble_context(
    user_id: str,
    app_name: str,
    environment: str,
    session_id: str,
    current_turn: str
) -> AssembledContext
```

Input validation requirements:
- `user_id` must match a known user record — reject unknown users
- `app_name` must match a registered app config — reject unknown apps
- `environment` must be one of: dev, stage, prod
- `session_id` must be a valid active session — reject expired sessions
- `current_turn` is untrusted input — treat as potentially adversarial

Return type `AssembledContext` must include:
- `config: ResolvedConfig` — merged config with source_layer annotations
- `retrieved_facts: list[Fact]` — top-N semantic facts
- `retrieved_episodes: list[Episode]` — top-M episodes
- `session_state: SessionState` — current working memory
- `safety_flags: list[str]` — any safety violations detected during assembly
- `context_budget_tokens: int` — estimated tokens consumed

## Memory Write API

```
write_memory(
    proposed_fact: FactProposal,
    session_id: str,
    user_id: str
) -> WriteResult
```

`WriteResult` must include:
- `status: Literal["written", "pending_review", "rejected"]`
- `reason: str` — why it was rejected or held
- `audit_id: str` — ID of the audit log entry
- `fact_id: str | None` — set if written
