"""
Artifact scanner — discovers AI assistant artifacts across all known locations.

AI-agnostic: scans for artifacts from Claude, ChatGPT, Copilot, Cursor,
and any custom paths the user configures.

This module knows WHERE to look. The engine decides WHAT to do.
"""

from __future__ import annotations

from pathlib import Path
from typing import Iterator

from .config import ScanTarget


# Known AI assistant artifact locations (expandable)
KNOWN_ARTIFACT_LOCATIONS: dict[str, list[ScanTarget]] = {
    "claude": [
        ScanTarget(
            path="~/.claude/subagents",
            pattern="*.jsonl",
            description="Claude subagent conversation logs",
        ),
        ScanTarget(
            path="~/.claude/projects",
            pattern="**/memory/**/*",
            recursive=True,
            description="Claude project memory files",
        ),
        ScanTarget(
            path="~/.claude/todos",
            pattern="*.json",
            description="Claude task artifacts",
        ),
        ScanTarget(
            path="~/.claude",
            pattern="history.jsonl",
            recursive=False,
            description="Claude conversation history",
        ),
    ],
    "chatgpt": [
        ScanTarget(
            path="~/Library/Application Support/com.openai.chat",
            pattern="**/*.json",
            description="ChatGPT desktop app cache (macOS)",
        ),
    ],
    "cursor": [
        ScanTarget(
            path="~/.cursor",
            pattern="**/*.jsonl",
            description="Cursor AI conversation logs",
        ),
    ],
    "copilot": [
        ScanTarget(
            path="~/.config/github-copilot",
            pattern="**/*.json",
            description="GitHub Copilot cache",
        ),
    ],
    "generic": [
        ScanTarget(
            path="~/.local/share/ai-memory",
            pattern="*",
            description="Generic AI memory store",
        ),
    ],
}


def discover_installed_assistants() -> dict[str, list[ScanTarget]]:
    """
    Auto-detect which AI assistants have artifacts on this system.
    Returns only the ones with existing directories.
    """
    found: dict[str, list[ScanTarget]] = {}

    for name, targets in KNOWN_ARTIFACT_LOCATIONS.items():
        existing = []
        for t in targets:
            if t.resolve().exists():
                existing.append(t)
        if existing:
            found[name] = existing

    return found


def iter_artifacts(targets: list[ScanTarget]) -> Iterator[Path]:
    """
    Yield all artifact files matching the given scan targets.
    Skips already-compressed (.zst) and encrypted (.age) files.
    """
    skip_suffixes = {".zst", ".age", ".tmp"}

    for target in targets:
        base = target.resolve()
        if not base.exists():
            continue

        pattern = f"**/{target.pattern}" if target.recursive else target.pattern

        for match in base.glob(pattern):
            if match.is_file() and match.suffix not in skip_suffixes:
                yield match
