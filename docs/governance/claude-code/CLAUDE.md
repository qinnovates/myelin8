# SIEMPLE-AI Reference Project

## Stack
YAML/JSON schemas · Markdown documentation · Bash hooks · semgrep

## Conventions
- Schema files: YAML with $schema and required fields annotated
- Layer config files: YAML with `layer:` frontmatter
- Memory stores: JSONL, one record per line, validated against schemas/
- Documentation: Markdown, factual claims only, no spec inflation
- Imperative voice in all rule files: "NEVER X" not "we avoid X"

## Commands
```
# Validate a memory file against its schema
python3 -m jsonschema -i <file>.jsonl schemas/fact.schema.yaml

# Find all imperative rules across config
grep -rn "NEVER\|ALWAYS\|MUST\|SHALL" .claude/rules/ CLAUDE.md

# Run hook health check
bash claude-code/hooks/post-tool-use.sh --test

# Audit rule contradictions
grep -n "NEVER" .claude/rules/*.md | sort
grep -n "ALWAYS" .claude/rules/*.md | sort
```

## Imports
@import claude-code/rules/security.md
@import claude-code/rules/consistency.md
@import claude-code/rules/error-handling.md
@import claude-code/rules/testing.md
@import claude-code/rules/api-design.md
@import claude-code/rules/verbose-permissions.md

## Prohibitions
NEVER write PII or credentials to any memory store or log file.
NEVER merge instructions, preferences, facts, and session state into one file.
NEVER treat CLAUDE.md as the security boundary — hooks enforce, CLAUDE.md advises.
NEVER inject full memory stores into context — retrieve top-N by relevance.
NEVER write inferred facts with confidence > 0.8.
NEVER allow downstream layers to override `immutable: true` fields.
NEVER store security constraints in semantic memory — they live in Layer 1 and hooks.
