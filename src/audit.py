"""
Minimal audit logger for Engram operations.

Logs: timestamp, operation, tier, short artifact hash, outcome.

NEVER logs: file paths, filenames, key sources, query strings,
content, keywords, summaries, usernames, private keys, secrets.

All log lines pass through PII/secret detection before writing.
If a line matches a secret pattern, it is BLOCKED and replaced
with a redaction notice.

Format: one line per event, append-only, 0600 permissions.
"""

from __future__ import annotations

import os
import re
import time
from pathlib import Path


# 10 MB cap — prevents disk fill under adversarial conditions
MAX_AUDIT_BYTES = 10 * 1024 * 1024

# PII and secret detection patterns — if ANY match, the line is blocked
_SECRET_PATTERNS = [
    re.compile(r'AGE-SECRET-KEY-[A-Z0-9]+', re.IGNORECASE),       # age private key
    re.compile(r'age1[a-z0-9]{58}'),                                # age public key (still metadata)
    re.compile(r'-----BEGIN .* KEY-----'),                          # PEM keys
    re.compile(r'ssh-(rsa|ed25519|ecdsa)\s+\S{20,}'),             # SSH keys
    re.compile(r'AKIA[0-9A-Z]{16}'),                               # AWS access key
    re.compile(r'sk-[a-zA-Z0-9]{20,}'),                            # OpenAI/Anthropic API key
    re.compile(r'ghp_[a-zA-Z0-9]{36,}'),                           # GitHub PAT
    re.compile(r'[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}'),# Email addresses
    re.compile(r'/Users/[a-zA-Z0-9._-]+/'),                        # macOS home paths (PII)
    re.compile(r'/home/[a-zA-Z0-9._-]+/'),                         # Linux home paths (PII)
    re.compile(r'password|passwd|secret|token|credential',
               re.IGNORECASE),                                      # Secret keywords in values
]


def _contains_secret(line: str) -> bool:
    """Check if a log line contains any secret or PII pattern."""
    for pattern in _SECRET_PATTERNS:
        if pattern.search(line):
            return True
    return False


class AuditLogger:
    """Append-only audit logger with PII/secret detection."""

    def __init__(self, log_dir: Path):
        self.log_path = log_dir / "audit.log"
        self._ensure_log()

    def _ensure_log(self) -> None:
        """Create log file with restricted permissions if it doesn't exist."""
        if not self.log_path.exists():
            self.log_path.parent.mkdir(parents=True, exist_ok=True)
            fd = os.open(
                str(self.log_path),
                os.O_CREAT | os.O_WRONLY, 0o600,
            )
            os.close(fd)

    def _write(self, line: str) -> None:
        """Append a line to the audit log. Blocks lines containing secrets/PII."""
        try:
            # PII/secret gate — block any line that matches a secret pattern
            if _contains_secret(line):
                line = f"{self._ts()} REDACTED — log line contained potential secret/PII"

            if self.log_path.exists() and self.log_path.stat().st_size > MAX_AUDIT_BYTES:
                return  # Log full — silently drop. User should rotate.
            with open(self.log_path, "a") as f:
                f.write(line + "\n")
        except OSError:
            pass  # Audit logging must never break the operation

    def _ts(self) -> str:
        return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

    def tier(self, from_tier: str, to_tier: str,
             artifact_hash: str, ratio: float) -> None:
        """Log a tier transition."""
        h = artifact_hash[:6]
        self._write(f"{self._ts()} TIER {from_tier}>{to_tier} {ratio:.1f}x {h}")

    def recall(self, tier: str, artifact_hash: str) -> None:
        """Log an artifact recall to hot tier."""
        self._write(f"{self._ts()} RECALL {tier} {artifact_hash[:6]}")

    def encrypt(self, tier: str, artifact_hash: str, ok: bool) -> None:
        """Log an encryption event."""
        status = "ok" if ok else "fail"
        self._write(f"{self._ts()} ENCRYPT {tier} {artifact_hash[:6]} {status}")

    def decrypt(self, tier: str, artifact_hash: str, ok: bool) -> None:
        """Log a decryption event."""
        status = "ok" if ok else "fail"
        self._write(f"{self._ts()} DECRYPT {tier} {artifact_hash[:6]} {status}")

    def rotate(self, tier: str, old_gen: int, new_gen: int, count: int) -> None:
        """Log a key rotation."""
        self._write(
            f"{self._ts()} ROTATE {tier} gen{old_gen}>{new_gen} {count}headers"
        )

    def search(self, result_count: int) -> None:
        """Log a search operation (no query content)."""
        self._write(f"{self._ts()} SEARCH {result_count}hits")

    def error(self, operation: str, short_hash: str = "") -> None:
        """Log an operation failure."""
        h = short_hash[:6] if short_hash else "?"
        self._write(f"{self._ts()} ERROR {operation} {h}")
