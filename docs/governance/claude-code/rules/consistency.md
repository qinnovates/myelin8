---
applies_to:
  - "**/*.yaml"
  - "**/*.yml"
  - "**/*.json"
  - "**/*.jsonl"
  - "**/*.md"
scope: "Activates when creating or editing schema, config, or documentation files"
---

# Consistency Rules

## YAML Files
ALWAYS include `layer:` frontmatter in config files.
ALWAYS include `version:` in config files.
ALWAYS annotate `overridable: true/false` on every field that downstream layers interact with.
NEVER mix tabs and spaces — use 2-space indentation.
NEVER use unquoted strings that could be interpreted as YAML special values (true, false, null, 1.0).

## JSONL Memory Files
ALWAYS one JSON object per line — no pretty-printing.
ALWAYS include required fields from the schema (id, namespace, type, key, value, confidence, source, created_at, updated_at, sensitivity).
NEVER include PII values even as examples — use placeholder text.
NEVER include confidence > 0.8 for source: inferred facts.

## Schema Files
ALWAYS include `$schema` reference.
ALWAYS include `additionalProperties: false` to prevent schema drift.
ALWAYS include `description` on every field.
ALWAYS include `examples` on at least the `id` and `key` fields.

## Documentation
NEVER state theoretical/unvalidated work as established fact.
ALWAYS use imperative voice for rules: "NEVER X" not "we avoid X" or "X is bad practice."
NEVER add documentation to files you didn't change.
ALWAYS include the SIEM analogy when explaining a concept that maps to SIEM architecture.
