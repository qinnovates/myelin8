# SOC Alert Triage Prompt Template
# Use: When analyzing a security alert for the SOC copilot app.
# Layer: App/Domain (Layer 2) — procedural memory for soc_copilot

---

## Alert Triage: {alert_name}

**Alert Details:**
- Source: {siem_source}
- Time: {alert_timestamp}
- Host: {hostname}
- User: {username}
- Raw event: {raw_event_data}

---

## Triage Output Format

### Verdict
`{BENIGN | SUSPICIOUS | MALICIOUS | NEEDS_INVESTIGATION}` — Confidence: {0.0-1.0}

### Severity
`{LOW | MEDIUM | HIGH | CRITICAL}`

### MITRE ATT&CK
Technique: {T####.###} — {Technique Name}
Tactic: {Tactic Name}

### Reasoning
(2-3 sentences maximum. Lead with the strongest evidence for your verdict.)

### Context Signals
- Baseline behavior: {normal for this user/host? deviation?}
- Related alerts: {any correlated activity in the last 24h?}
- Known FP patterns: {does this match documented FP patterns?}
- Threat intel: {any IOC matches?}

### Recommended Action
`{CLOSE_AS_BENIGN | MONITOR | INVESTIGATE | ESCALATE | CONTAIN}`

Specific next step: {one sentence}

### Validation Queries
```spl
# SPL / KQL queries to validate or expand this investigation
{siem_query_1}

{siem_query_2}
```

---

## Triage Discipline

- Verdict first, reasoning second. Analysts don't have time for preamble.
- If confidence < 0.3: output NEEDS_INVESTIGATION, not a guess.
- If the alert is in a pattern of known FPs: say so explicitly with the FP rate.
- Do not inflate severity. A low-severity finding is not medium because it's unclear.
- Missing context is not a reason to escalate — it's a reason to investigate.
