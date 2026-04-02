# Best Practices Checklist

> Use this when starting a new AI project, reviewing an existing setup, or doing a quarterly audit. Items are ordered by impact — fix the top items first.

---

## Tier 1: Critical (fix before going to production)

### CLAUDE.md Structure
- [ ] Root CLAUDE.md is under 60 lines
- [ ] Prohibitions stanza is the LAST stanza in the file
- [ ] All rules use imperative framing (`NEVER X`, `ALWAYS Y`) — no descriptive prose for constraints
- [ ] No version pins or stack references in the prohibitions stanza
- [ ] At least one `@import` pointing to domain rule files (not everything in one file)
- [ ] No rule duplicated in both CLAUDE.md and a domain rule file

### Security Boundary
- [ ] PostToolUse hook exists and is executable
- [ ] Hook runs semgrep (or equivalent) on all generated code files
- [ ] Hook returns non-zero exit code on violations
- [ ] Hook is NOT disabled in dev environment
- [ ] Security constraints exist in BOTH CLAUDE.md (advisory) AND the hook (enforcement)
- [ ] Security constraints are NOT stored in semantic memory

### Layer 1 Safety
- [ ] Safety fields in system config are marked `immutable: true` or `overridable: false`
- [ ] `safety.pii_redaction` cannot be overridden by any downstream layer
- [ ] `tool_policy.code_exec.mode: sandbox` cannot be overridden
- [ ] Immutability violations are logged to the audit trail
- [ ] Context assembly pipeline validates immutability before completing the merge

---

## Tier 2: High Impact (fix within the first week)

### Memory Architecture
- [ ] Semantic memory uses structured JSONL with confidence scores, not a flat markdown file
- [ ] Every fact has a `source` field: `explicit | inferred | imported`
- [ ] Inferred facts have a confidence ceiling of 0.8
- [ ] Inferred facts have a TTL (`expires_at` set, maximum 180 days)
- [ ] Explicit facts have `last_confirmed_at` tracking
- [ ] PII is never stored in any memory field value
- [ ] Credentials are never stored in any memory field value

### Retrieval
- [ ] Memory injection uses top-N retrieval, NOT full-store injection
- [ ] Retrieval budget is defined: max 5-8 semantic facts, max 3-5 episodes
- [ ] Minimum confidence threshold for retrieval: 0.5 (configurable)
- [ ] Turn history is windowed (last 10-20 turns), not unlimited

### Write Policy
- [ ] Single-session inferences go to `pending_memory_writes`, NOT directly to semantic memory
- [ ] Pending writes require user confirmation before persisting
- [ ] Every durable write produces an audit log entry
- [ ] Contradiction detection runs before every write
- [ ] Model cannot unilaterally modify its own instructions (CLAUDE.md, rules files)

---

## Tier 3: Medium Impact (fix within the first sprint)

### Config Layer Separation
- [ ] System config (Layer 1) is separate from app config (Layer 2)
- [ ] Environment config (Layer 3) exists and differs appropriately between dev and prod
- [ ] User preferences (Layer 4) are stored separately from session state (Layer 5)
- [ ] Instructions, preferences, facts, and state are in separate stores (not one blob)
- [ ] Session state has `expires_at` — no unlimited session TTLs

### Domain Rule Files
- [ ] Rules files have YAML frontmatter with `applies_to:` path scoping
- [ ] Domain files are under 50 lines each
- [ ] Total lines across all rule files is under 500
- [ ] Each domain file covers one concern (security, naming, testing, etc.)

### Audit Trail
- [ ] Config change audit log exists with timestamps and change authors
- [ ] Memory write audit log exists with trigger phrases and confidence levels
- [ ] Audit logs are append-only (no modification of existing entries)
- [ ] Audit logs are separate files from the memory stores they track

---

## Tier 4: Lower Impact / Quality of Life (quarterly cadence)

### Adherence Testing
- [ ] Quarterly adherence test run (`claude "Summarize my CLAUDE.md in 5 points"`)
- [ ] Contradiction scan run quarterly (`grep -n "NEVER\|ALWAYS" .claude/rules/*.md`)
- [ ] Total line count verified under thresholds
- [ ] Version pins verified against current stable releases
- [ ] Stale `@import` references verified to still resolve

### Memory Hygiene
- [ ] Low-confidence facts (< 0.3) purged
- [ ] Expired facts removed from active store
- [ ] Old inferred facts (90+ days without confirmation) reviewed and either confirmed or decayed
- [ ] Episode store reviewed: compaction run, importance scores checked
- [ ] Duplicate facts in same namespace identified and merged

### Hook Maintenance
- [ ] Hook self-test run (`bash post-tool-use.sh --test`)
- [ ] Semgrep rules reviewed for stale patterns
- [ ] New common violation patterns added as rules
- [ ] False-positive rate verified to be < 5%

---

## Quick-Start Checklist (Minimum Viable Setup)

If you're starting from scratch and want the most impactful 20% of work:

1. **Write a 60-line CLAUDE.md** with five stanzas. Prohibitions at the bottom. Imperative framing.
2. **Create three domain rule files:** security, error-handling, conventions. Each under 40 lines.
3. **Install the PostToolUse hook.** Even a minimal semgrep scan of generated Python/JS is better than none.
4. **Create a semantic_memory.jsonl** for user preferences. Schema: `{id, key, value, confidence, source, created_at}` — you can add fields as you mature.
5. **Set a retrieval budget.** Hard-code `max_retrieved_facts: 5` in your context assembly. You can tune it later.

That's it. You'll be ahead of 80% of teams in AI configuration quality.

---

## Anti-Patterns to Check For

Run these checks any time you onboard a new project or inherit someone else's AI configuration:

```bash
# 1. God file check
wc -l CLAUDE.md .claude/rules/*.md 2>/dev/null || wc -l CLAUDE.md
# Warn if any single file > 200 lines

# 2. Prohibitions position check
grep -n "Prohibitions\|NEVER\|ALWAYS" CLAUDE.md | head -5
# Warn if prohibitions appear before line 40

# 3. Descriptive framing check
grep -i "generally\|usually\|we try\|we prefer\|the team" CLAUDE.md .claude/rules/*.md
# Each result is a candidate for imperative rewriting

# 4. Memory type mixing check
# Does the memory file contain both user preferences and security constraints?
# If yes: separate them immediately

# 5. Missing TTL check
python3 -c "
import json
for line in open('semantic_memory.jsonl'):
    f = json.loads(line)
    if f.get('source') == 'inferred' and not f.get('expires_at'):
        print(f'WARNING: inferred fact with no TTL: {f[\"id\"]}')
"

# 6. Hook executable check
ls -la .claude/hooks/post-tool-use.sh
# Must show -rwxr-xr-x (executable)
```
