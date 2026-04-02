# /slop-check

Run this before any PR or commit to catch configuration quality issues.

## Checks

### 1. CLAUDE.md / Rules Adherence
```bash
# Verify CLAUDE.md is under 60 lines
wc -l claude-code/CLAUDE.md
# Should output: < 60

# Test adherence
claude "Summarize claude-code/CLAUDE.md in 5 points covering the most important constraints"
# Review: does the model reproduce all prohibitions correctly?
```

### 2. Rule Contradictions
```bash
grep -n "NEVER" claude-code/rules/*.md claude-code/CLAUDE.md | sort
grep -n "ALWAYS" claude-code/rules/*.md claude-code/CLAUDE.md | sort
# Review manually for conflicting NEVER/ALWAYS pairs on the same topic
```

### 3. Schema Validity
```bash
# Every .yaml in schemas/ must parse without error
for f in schemas/*.yaml; do python3 -c "import yaml; yaml.safe_load(open('$f'))" && echo "$f: OK"; done

# Every .jsonl in templates/ must be valid JSON per line
for f in $(find templates/ -name "*.jsonl"); do
  while IFS= read -r line; do
    echo "$line" | python3 -c "import sys,json; json.load(sys.stdin)" || echo "INVALID LINE in $f: $line"
  done < "$f"
done
```

### 4. Immutability Annotations
```bash
# Every field in safety.yaml should have overridable: false or be under immutable: true parent
grep -c "overridable" templates/system/safety.yaml
# If count is 0: safety fields are missing their immutability annotations
```

### 5. PII Scan
```bash
# No email addresses in any YAML/JSON/JSONL
grep -rn '[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]\{2,\}' templates/ schemas/ examples/
# Should return no results

# No phone numbers
grep -rn '\b[0-9]\{3\}[-.\s][0-9]\{3\}[-.\s][0-9]\{4\}\b' templates/ schemas/
# Should return no results
```

### 6. Hook Health
```bash
bash claude-code/hooks/post-tool-use.sh --test
# Should detect the known violation patterns
# Should NOT false-positive on clean test cases
```

## Pass Criteria
- CLAUDE.md under 60 lines ✓
- No contradictory NEVER/ALWAYS pairs ✓
- All schema files parse cleanly ✓
- All JSONL example files contain valid JSON ✓
- No PII in template files ✓
- Hook detects violations, passes clean code ✓
