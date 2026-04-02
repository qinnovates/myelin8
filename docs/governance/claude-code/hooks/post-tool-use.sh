#!/usr/bin/env bash
# PostToolUse Hook — The Actual Security Boundary
#
# This hook runs AFTER the model writes code, BEFORE the output is accepted.
# It cannot be "forgotten." It cannot be crowded out of the context window.
# It cannot be overridden by a clever prompt.
# This is why CLAUDE.md is advisory and this hook is the enforcement layer.
#
# Usage:
#   Triggered automatically by Claude Code's PostToolUse hook mechanism.
#   Manual test: bash post-tool-use.sh --test
#
# Configuration:
#   SEMGREP_RULES_PATH — path to semgrep rules file (default: rules/semgrep.yaml in same dir)
#   BANDIT_ENABLED — set to "true" to also run bandit on Python files (default: false)
#   HOOK_STRICT — set to "true" to block on MEDIUM findings (default: false, only blocks HIGH+CRITICAL)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SEMGREP_RULES="${SEMGREP_RULES_PATH:-${SCRIPT_DIR}/semgrep.yaml}"
BANDIT_ENABLED="${BANDIT_ENABLED:-false}"
HOOK_STRICT="${HOOK_STRICT:-false}"
EXIT_CODE=0

# ────────────────────────────────────────────────────────────────
# Test mode: validate hook is working with known violation patterns
# ────────────────────────────────────────────────────────────────
if [[ "${1:-}" == "--test" ]]; then
    echo "=== PostToolUse Hook Self-Test ==="
    PASS=0
    FAIL=0

    # Test 1: shell=True should be caught
    TMP=$(mktemp /tmp/hook_test_XXXXXX.py)
    echo 'import subprocess; subprocess.run(["ls"], shell=True)' > "$TMP"
    if semgrep --config "$SEMGREP_RULES" "$TMP" --quiet 2>/dev/null | grep -q "shell=True"; then
        echo "✓ PASS: shell=True detected"
        ((PASS++))
    else
        echo "✗ FAIL: shell=True NOT detected"
        ((FAIL++))
    fi
    rm -f "$TMP"

    # Test 2: Clean code should not trigger
    TMP=$(mktemp /tmp/hook_test_XXXXXX.py)
    echo 'import subprocess; subprocess.run(["ls", "-la"], shell=False)' > "$TMP"
    if semgrep --config "$SEMGREP_RULES" "$TMP" --quiet 2>/dev/null | grep -q "shell=True"; then
        echo "✗ FAIL: False positive on clean code"
        ((FAIL++))
    else
        echo "✓ PASS: Clean code passes"
        ((PASS++))
    fi
    rm -f "$TMP"

    # Test 3: Hardcoded API key pattern should be caught
    TMP=$(mktemp /tmp/hook_test_XXXXXX.py)
    echo 'api_key = "sk-abcdefghijklmnopqrstuvwx12345678"' > "$TMP"
    if semgrep --config "$SEMGREP_RULES" "$TMP" --quiet 2>/dev/null | grep -q "hardcoded"; then
        echo "✓ PASS: Hardcoded API key detected"
        ((PASS++))
    else
        echo "✗ FAIL: Hardcoded API key NOT detected"
        ((FAIL++))
    fi
    rm -f "$TMP"

    # Test 4: SQL string concatenation should be caught
    TMP=$(mktemp /tmp/hook_test_XXXXXX.py)
    echo 'query = "SELECT * FROM users WHERE id = " + user_id' > "$TMP"
    if semgrep --config "$SEMGREP_RULES" "$TMP" --quiet 2>/dev/null | grep -q "sql"; then
        echo "✓ PASS: SQL concatenation detected"
        ((PASS++))
    else
        echo "✗ FAIL: SQL concatenation NOT detected"
        ((FAIL++))
    fi
    rm -f "$TMP"

    echo ""
    echo "Results: ${PASS} passed, ${FAIL} failed"
    [[ $FAIL -eq 0 ]] && echo "Hook is healthy." || echo "Hook has failures — fix before relying on it."
    exit $FAIL
fi

# ────────────────────────────────────────────────────────────────
# Standard mode: called by Claude Code with tool output file path
# ────────────────────────────────────────────────────────────────
TARGET_FILE="${1:-}"

if [[ -z "$TARGET_FILE" ]]; then
    # No file argument — read from stdin or check context
    # This handles the case where Claude Code passes the written content differently
    echo "PostToolUse: no target file specified, skipping scan." >&2
    exit 0
fi

if [[ ! -f "$TARGET_FILE" ]]; then
    echo "PostToolUse: target file not found: $TARGET_FILE" >&2
    exit 0
fi

# Only scan file types we have rules for
FILE_EXT="${TARGET_FILE##*.}"
SCANNABLE_EXTENSIONS=("py" "ts" "js" "sh" "bash")
SHOULD_SCAN=false
for ext in "${SCANNABLE_EXTENSIONS[@]}"; do
    if [[ "$FILE_EXT" == "$ext" ]]; then
        SHOULD_SCAN=true
        break
    fi
done

if [[ "$SHOULD_SCAN" == "false" ]]; then
    exit 0
fi

# ────────────────────────────────────────────────────────────────
# Run semgrep
# ────────────────────────────────────────────────────────────────
if ! command -v semgrep &>/dev/null; then
    echo "WARNING: semgrep not installed. PostToolUse security scanning is DISABLED." >&2
    echo "Install: pip install semgrep" >&2
    exit 0
fi

echo "PostToolUse: scanning ${TARGET_FILE}..."

SEMGREP_OUTPUT=$(semgrep --config "$SEMGREP_RULES" "$TARGET_FILE" \
    --json --quiet 2>/dev/null || true)

HIGH_FINDINGS=$(echo "$SEMGREP_OUTPUT" | python3 -c "
import sys, json
data = json.load(sys.stdin)
findings = data.get('results', [])
high = [f for f in findings if f.get('extra', {}).get('severity', '').upper() in ('ERROR', 'WARNING')]
for f in high:
    print(f\"{f['path']}:{f['start']['line']} [{f['extra']['severity']}] {f['check_id']}: {f['extra']['message']}\")
sys.exit(len(high))
" 2>/dev/null)
SEMGREP_HIGH_COUNT=$?

if [[ $SEMGREP_HIGH_COUNT -gt 0 ]]; then
    echo ""
    echo "╔══════════════════════════════════════════════════════════════════╗"
    echo "║  PostToolUse VIOLATION — Code blocked pending remediation        ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo ""
    echo "Violations found in: ${TARGET_FILE}"
    echo ""
    echo "$HIGH_FINDINGS"
    echo ""
    echo "Fix the violations above before this code will be accepted."
    echo "These rules exist because CLAUDE.md is advisory; this hook is enforcement."
    EXIT_CODE=1
fi

# ────────────────────────────────────────────────────────────────
# Run bandit (Python only, if enabled)
# ────────────────────────────────────────────────────────────────
if [[ "$BANDIT_ENABLED" == "true" && "$FILE_EXT" == "py" ]]; then
    if command -v bandit &>/dev/null; then
        BANDIT_OUTPUT=$(bandit -r "$TARGET_FILE" -f json -ll 2>/dev/null || true)
        BANDIT_HIGH=$(echo "$BANDIT_OUTPUT" | python3 -c "
import sys, json
data = json.load(sys.stdin)
issues = data.get('results', [])
high = [i for i in issues if i.get('issue_severity') in ('HIGH', 'MEDIUM')]
for i in high:
    print(f\"{i['filename']}:{i['line_number']} [{i['issue_severity']}] {i['issue_text']}\")
sys.exit(len(high))
" 2>/dev/null)
        BANDIT_COUNT=$?
        if [[ $BANDIT_COUNT -gt 0 ]]; then
            echo ""
            echo "Bandit findings:"
            echo "$BANDIT_HIGH"
            EXIT_CODE=1
        fi
    fi
fi

exit $EXIT_CODE
