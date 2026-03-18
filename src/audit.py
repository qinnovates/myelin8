"""
Minimal audit logger for Engram operations.

Logs: timestamp, operation, tier, short artifact hash, outcome.
Does NOT log: file paths, filenames, key sources, query strings,
content, keywords, summaries, usernames.

Format: one line per event, append-only, 0600 permissions.
An attacker who reads the audit log learns that you tiered 37 things
and recalled one. They don't learn what any of them are.
"""

from __future__ import annotations

import os
import time
from pathlib import Path


class AuditLogger:
    """Append-only audit logger with minimal metadata."""

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
        """Append a single line to the audit log."""
        try:
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
