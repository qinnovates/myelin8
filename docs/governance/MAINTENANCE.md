# Maintenance Guide: Quarterly Audit and Drift Detection

> Configuration drift is how production SIEM deployments become unreliable. The same applies to AI context systems. Catch it before it compounds.

---

## Quarterly Audit Checklist

Run this every 90 days. Block it on the calendar. Drift that goes unaddressed for a year is a refactor, not a cleanup.

### 1. CLAUDE.md / Rules File Audit

```bash
# Find all rules files
find . -name "*.md" -path "*/.claude/*" | sort

# Count total lines across all config files
find . -name "*.md" -path "*/.claude/*" -exec wc -l {} + | tail -1

# Find all imperative rules
grep -rn "NEVER\|ALWAYS\|MUST\|SHALL NOT\|REQUIRED" .claude/rules/ CLAUDE.md | sort

# Spot potential contradictions (same verb, different targets)
grep -n "NEVER" .claude/rules/*.md CLAUDE.md
grep -n "ALWAYS" .claude/rules/*.md CLAUDE.md
```

**Check for:**
- [ ] Total line count under 500 across all CLAUDE.md and rules files
- [ ] No contradictory NEVER/ALWAYS pairs
- [ ] Every `@import` reference points to an existing file
- [ ] Version pins are still current (Python 3.X, Node 20, etc.)
- [ ] No rule duplicated in both CLAUDE.md and a domain rules file

**Adherence test:**
```bash
claude "Summarize my CLAUDE.md and all rules files in 10 points covering the most important constraints"
```
If the model misses critical constraints or gets them wrong, the files need restructuring.

### 2. Memory Audit

```bash
# Count semantic memory entries by namespace
jq -r '.namespace' semantic_memory.jsonl | sort | uniq -c

# Find low-confidence facts that should be reviewed
jq 'select(.confidence < 0.5)' semantic_memory.jsonl

# Find expired facts that weren't cleaned up
jq --arg now "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  'select(.expires_at != null and .expires_at < $now)' semantic_memory.jsonl

# Find inferred facts older than 90 days (may be stale)
jq --arg cutoff "$(date -u -d '90 days ago' +%Y-%m-%dT%H:%M:%SZ)" \
  'select(.source == "inferred" and .created_at < $cutoff)' semantic_memory.jsonl
```

**Check for:**
- [ ] No facts with `confidence < 0.3` — these are noise
- [ ] No expired facts still in the active store
- [ ] No PII in fact values (email addresses, phone numbers, location data)
- [ ] Inferred facts older than 90 days reviewed and either confirmed or deleted
- [ ] No duplicate facts covering the same key in the same namespace

### 3. Schema Drift Detection

```bash
# Validate all facts against the schema
for f in $(find . -name "semantic_memory.jsonl"); do
  python3 -c "
import jsonschema, json, yaml
schema = yaml.safe_load(open('schemas/fact.schema.yaml'))
for line in open('$f'):
    fact = json.loads(line)
    jsonschema.validate(fact, schema)
print('$f: OK')
"
done

# Check for unknown fields in fact files (schema drift)
jq -r 'keys[]' semantic_memory.jsonl | sort -u
```

### 4. Hook Health Check

```bash
# Verify hooks are installed and executable
ls -la .claude/hooks/
# Check hook exit codes on a known-bad test case
echo "import subprocess; subprocess.run('ls', shell=True)" > /tmp/test_shell_true.py
bash .claude/hooks/post-tool-use.sh /tmp/test_shell_true.py
echo "Hook exit code: $?"  # Should be non-zero (violation detected)
rm /tmp/test_shell_true.py
```

**Check for:**
- [ ] `post-tool-use.sh` exists and is executable (`chmod +x`)
- [ ] Semgrep rules file exists and is not empty
- [ ] Hook catches known violations (run the test above)
- [ ] Hook does not false-positive on clean code
- [ ] Hook is registered in Claude Code settings or equivalent

### 5. Audit Log Rotation

```bash
# Check audit log sizes
wc -l audit/config_changes.jsonl audit/memory_writes.jsonl

# Archive logs older than 90 days
jq --arg cutoff "$(date -u -d '90 days ago' +%Y-%m-%dT%H:%M:%SZ)" \
  'select(.ts < $cutoff)' audit/config_changes.jsonl > audit/archive/config_changes_$(date +%Y%m%d).jsonl

# Remove archived entries from active log
jq --arg cutoff "$(date -u -d '90 days ago' +%Y-%m-%dT%H:%M:%SZ)" \
  'select(.ts >= $cutoff)' audit/config_changes.jsonl > /tmp/config_changes_trimmed.jsonl
mv /tmp/config_changes_trimmed.jsonl audit/config_changes.jsonl
```

---

## Drift Detection Patterns

Drift in AI context systems manifests differently than SIEM drift. Here's what to watch for:

### Adherence Drift

**Symptom:** The model stops following a rule that it followed reliably three months ago.

**Cause:** Usually one of:
1. The rules file grew too long and the rule is now buried
2. A new conflicting rule was added
3. The rule uses descriptive rather than imperative framing
4. The rule was in a section that's no longer @imported correctly

**Detection:**
```bash
# Test specific rule adherence
claude "Write a Python function that runs 'ls' as a shell command"
# Expected: model uses subprocess with shell=False or argument list
# If model uses shell=True: adherence drift on security rule
```

**Fix:** Move the rule to the prohibitions stanza (bottom of CLAUDE.md), rewrite in imperative form, ensure it's not contradicted.

### Memory Inflation

**Symptom:** Semantic memory grows without bound. Retrieval starts surfacing low-relevance facts. Response quality degrades.

**Cause:** Write policy is too permissive. Model is writing facts for temporary states, single-session observations, or speculative inferences.

**Detection:**
```bash
# Memory entry count over time
jq -r '.created_at | split("T")[0]' semantic_memory.jsonl | sort | uniq -c
# If entries/day is increasing month-over-month: inflation is occurring
```

**Fix:** Tighten the write policy. Add a review gate for `source: inferred` facts before they persist.

### Precedence Regression

**Symptom:** A user preference is being ignored. An app-level default is overriding a user setting that should win.

**Cause:** Usually a schema change that made a field non-overridable incorrectly, or a new layer being inserted at the wrong precedence level.

**Detection:** Run the precedence resolution test (see [`docs/precedence-resolution.md`](precedence-resolution.md)).

### Hook Rot

**Symptom:** The PostToolUse hook stops blocking violations. Security constraints that should be enforced are being silently passed.

**Cause:** Semgrep rule pattern became stale (caught `shell=True` but not `shell = True` after a code formatter change). Or the hook was accidentally removed/disabled.

**Detection:** Run the hook health check above monthly, not just quarterly.

---

## Version Pinning Policy

Every version reference in CLAUDE.md and rules files should be audited against current reality:

```bash
# Find all version references
grep -rn ">=\|==\|^[0-9]\+\.[0-9]" .claude/rules/ CLAUDE.md
```

For each version pin found:
1. Is the minimum version still below the current LTS/stable release? If not, update.
2. Is the pin still necessary, or is the relevant issue long since resolved?
3. Does a new major version exist that changes the relevant behavior? If so, test and update.

---

## The 90-Day Rule for Inferred Facts

Any semantic memory fact with `source: inferred` that hasn't been confirmed by an explicit user statement within 90 days should be:

1. Demoted to lower confidence (multiply by 0.5)
2. Flagged for review
3. Deleted if confidence drops below 0.3

This prevents the memory store from accumulating stale beliefs. A fact about a user's preference that was inferred from a single session 8 months ago is not reliable — it may have been a one-time thing, or the user's preferences may have changed.

Implement this as a scheduled job:
```python
# pseudocode — run weekly
for fact in semantic_memory.get_all():
    if fact.source == "inferred":
        days_since_confirmation = (now - fact.last_confirmed_at).days
        if days_since_confirmation > 90:
            fact.confidence *= 0.5
            if fact.confidence < 0.3:
                semantic_memory.delete(fact.id)
                audit_log.write(event="memory_expired", fact_id=fact.id, reason="90_day_inferred_decay")
            else:
                audit_log.write(event="memory_confidence_decayed", fact_id=fact.id, new_confidence=fact.confidence)
```
