# /build-order

Reference build order for implementing features in this repo.
Follow this sequence. Do not skip phases.

## Phase 1: Schema First
Define the shape of any new data before writing implementation code.

- Add fields to the appropriate schema in `schemas/`
- Annotate every field: type, description, required vs optional, examples
- Add `additionalProperties: false`
- Verify the schema parses: `python3 -c "import yaml; yaml.safe_load(open('schemas/<file>.yaml'))"`

**Gate:** Schema is valid YAML and passes a manual review of required fields.

## Phase 2: Template
Implement the schema in the appropriate config layer template.

- Identify which layer: system, app, environment, user, or session
- Add the field to the correct template file in `templates/`
- Annotate `overridable: true/false` if it's a config field
- If it interacts with downstream layers: document the override behavior

**Gate:** Template file is valid YAML and explicitly handles the override/immutability case.

## Phase 3: Documentation
Update docs to explain the new field, layer, or behavior.

- If it's a new concept: add to `docs/architecture.md`
- If it's a memory behavior change: update `docs/memory-architecture.md`
- If it's a precedence change: update `docs/precedence-resolution.md`
- If it introduces a new anti-pattern or fixes one: update `docs/anti-patterns.md`
- No doc updates for purely additive template fields

**Gate:** Documentation is factual, uses imperative voice for rules, and doesn't inflate theoretical claims.

## Phase 4: Example
Add or update an example that demonstrates the new behavior.

- If context assembly is affected: update `examples/context-bundle.json`
- If memory lifecycle is affected: update `examples/memory-lifecycle.md`
- If precedence is affected: update `examples/precedence-resolution.md`

**Gate:** Example is concrete, correct, and exercises the new behavior end-to-end.

## Phase 5: Hook Update (if security-relevant)
If the new feature involves code execution, file writes, or trust boundaries:

- Add a semgrep rule to `claude-code/hooks/post-tool-use.sh`
- Test it: `bash claude-code/hooks/post-tool-use.sh --test`
- Verify it catches violations without false-positives on clean code

**Gate:** Hook test passes.

## Phase 6: Run /slop-check
Run the full quality gate before committing.

**Gate:** All /slop-check criteria pass.
