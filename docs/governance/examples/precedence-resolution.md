# Precedence Resolution — Worked Example

This walks through a complete precedence resolution for the `research_copilot` app in production with an example user. Follow each step to see exactly how the resolved config is computed.

---

## Setup

**Layers:**
1. `templates/system/defaults.yaml`
2. `templates/apps/research/default.yaml`
3. `templates/environments/prod.yaml`
4. `templates/users/example-user/preferences.yaml`
5. `templates/runtime/sessions/example-session.json` (session working memory)

---

## Step 1: Load System Layer

```yaml
persona.role: "assistant"                       # source: system
response.verbosity: medium                      # source: system, overridable: true
response.output_format: markdown                # source: system, overridable: true
safety.pii_redaction: true                      # source: system, IMMUTABLE
tool_policy.code_exec.mode: sandbox_only        # source: system, IMMUTABLE
retrieval.max_retrieved_facts: 5                # source: system, overridable: true
```

---

## Step 2: Apply App Layer (research_copilot)

Fields in the app layer that differ from the system layer:

| Field | System Value | App Override | Winner |
|-------|-------------|--------------|--------|
| `persona.role` | "assistant" | "Research analyst" | App (higher precedence) |
| `response.ask_clarifying_questions` | (unset) | "minimal" | App (new field) |
| `citations.verification_required` | (unset) | true | App (new field) |
| `retrieval.max_retrieved_facts` | 5 | 8 | App (overridable field) |

After app layer merge:
```yaml
persona.role: "Research analyst and technical advisor"  # source: app
response.verbosity: medium                              # source: system (app didn't touch)
response.ask_clarifying_questions: minimal              # source: app (new field)
citations.verification_required: true                  # source: app (new field)
retrieval.max_retrieved_facts: 8                       # source: app
safety.pii_redaction: true                             # source: system, IMMUTABLE (app cannot change)
```

---

## Step 3: Apply Environment Layer (prod)

| Field | Current Value | Prod Override | Winner |
|-------|--------------|---------------|--------|
| `model.primary` | (unset) | "claude-opus-4-6" | Prod (new field) |
| `retrieval.max_retrieved_facts` | 8 | 5 | Prod (overrides app — prod is more restrictive) |
| `observability.prompt_logging` | (unset) | "metadata_only" | Prod (new field) |

After environment layer merge:
```yaml
persona.role: "Research analyst and technical advisor"  # source: app
model.primary: "claude-opus-4-6"                       # source: environment
retrieval.max_retrieved_facts: 5                       # source: environment (overrides app's 8)
observability.prompt_logging: metadata_only            # source: environment
safety.pii_redaction: true                             # source: system, IMMUTABLE
```

Note: The research_copilot app requested `max_retrieved_facts: 8` for deeper research context, but prod's budget constraints (`max_retrieved_facts: 5`) won. This is correct behavior — environment-level resource constraints override app-level preferences.

---

## Step 4: Apply User Layer (example-user)

| Field | Current Value | User Override | Winner |
|-------|-------------|---------------|--------|
| `response.verbosity` | medium | medium | Tie (same value, user layer still recorded as source) |
| `response.output_format` | markdown | markdown | Tie |
| `response.tone` | (unset) | direct | User (new field) |
| `response.output_length` | (unset) | concise | User (new field) |
| `interests` | [] | ["neurosecurity", "BCI", ...] | User (additive field) |

After user layer merge:
```yaml
persona.role: "Research analyst and technical advisor"
model.primary: "claude-opus-4-6"
response.verbosity: medium                # source: user (confirmed, same as system default)
response.output_format: markdown
response.tone: direct                     # source: user
response.output_length: concise           # source: user
interests: ["neurosecurity", "BCI", "cybersecurity architecture", "iOS/Swift"]
safety.pii_redaction: true                # IMMUTABLE
```

---

## Step 5: Apply Session Layer

Session layer contributes working state, not config overrides (except temporary_overrides):

```yaml
session.objective: "Design memory write policy schema"   # session-only
session.active_files: ["schemas/...", "docs/..."]        # session-only
response.verbosity: high                                  # TEMPORARY OVERRIDE for this session
```

The `response.verbosity: high` from the session's `temporary_overrides` applies for this session only. It does NOT write back to the user layer. At session end, verbosity reverts to medium.

---

## Final Resolved Config

```yaml
persona.role: "Research analyst and technical advisor"   # layer: app
model.primary: "claude-opus-4-6"                        # layer: environment
response.verbosity: high                                 # layer: session (temporary)
response.output_format: markdown                        # layer: user
response.tone: direct                                   # layer: user
response.output_length: concise                         # layer: user
response.ask_clarifying_questions: minimal              # layer: app
citations.verification_required: true                  # layer: app
retrieval.max_retrieved_facts: 5                       # layer: environment
observability.prompt_logging: metadata_only            # layer: environment
interests: ["neurosecurity", "BCI", "cybersecurity architecture", "iOS/Swift"]  # layer: user (additive)
safety.pii_redaction: true                             # layer: system, IMMUTABLE
tool_policy.code_exec.mode: sandbox_only               # layer: system, IMMUTABLE
session.objective: "Design memory write policy schema" # layer: session, expires at session end
```

---

## What This Demonstrates

1. **Deterministic precedence**: every field has an unambiguous source layer. No mystery values.
2. **Immutability held**: `safety.pii_redaction` and `tool_policy.code_exec.mode` were never overridden despite 4 layers having the opportunity.
3. **Resource constraints win**: prod's `max_retrieved_facts: 5` overrode app's `8`. Resource policy is enforced at environment layer, not app layer.
4. **Session state is temporary**: `verbosity: high` applies this session only. The user's durable preference remains `medium`.
5. **Additive fields accumulate**: `interests` grew through additive merge. Neither app nor user overwrote the other's contributions.
