#!/usr/bin/env python3
"""phase-edit-guard.py

Claude Code PreToolUse hook for thin-workflow-harness.

Block Edit/Write/MultiEdit against source files while the current harness
phase is research / hearing / design. The point is purely *upstream*
prevention; harness L1-L4 gates are post-hoc checks and cannot stop an
agent from editing code mid-research.

Stdin (Claude Code PreToolUse format):
  {"tool_name": "Edit", "tool_input": {"file_path": "...", ...}, ...}

Exit codes:
  0 -> allow (pass-through)
  2 -> block (Claude Code surfaces stderr to the model)

Silent skip cases (exit 0):
  * tool is not Edit / Write / MultiEdit
  * HARNESS_HOME unset and no `.harness/` in cwd
  * `harness` binary not on PATH
  * `harness status` says "no runs found" or otherwise non-zero
  * current phase is not in {research, hearing, design}
  * target file extension is not a known source extension

Bypass:
  Set SKIP_PHASE_GUARD=1 in the environment.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

GUARDED_PHASES = {"research", "hearing", "design"}
SOURCE_EXTS = {".rs", ".py", ".ts", ".js", ".go", ".rb", ".java"}
EDIT_TOOLS = {"Edit", "Write", "MultiEdit"}

NODE_LINE_RE = re.compile(r"^node\s*:\s*\d+\s*/\s*\d+\s+(\S+)")


def harness_root() -> Path | None:
    """Return the directory containing `.harness/`, or None if not found."""
    env = os.environ.get("HARNESS_HOME")
    if env:
        p = Path(env)
        if (p / ".harness").is_dir():
            return p
        if p.name == ".harness" and p.is_dir():
            return p.parent
    cwd = Path.cwd()
    for parent in [cwd, *cwd.parents]:
        if (parent / ".harness").is_dir():
            return parent
    return None


def current_phase(root: Path) -> str | None:
    """Run `harness status` in `root` and parse the node id."""
    binary = shutil.which("harness") or shutil.which("harness.exe")
    if not binary:
        return None
    try:
        proc = subprocess.run(
            [binary, "status"],
            cwd=str(root),
            capture_output=True,
            text=True,
            timeout=5,
            encoding="utf-8",
            errors="replace",
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    for line in proc.stdout.splitlines():
        m = NODE_LINE_RE.match(line)
        if m:
            return m.group(1).strip().lower()
    return None


def extract_target(tool_name: str, tool_input: dict) -> list[str]:
    """Pull file paths out of the tool input. Best-effort."""
    paths: list[str] = []
    fp = tool_input.get("file_path")
    if isinstance(fp, str):
        paths.append(fp)
    return paths


def is_source(path_str: str) -> bool:
    ext = Path(path_str).suffix.lower()
    return ext in SOURCE_EXTS


def emit_block(phase: str, targets: list[str]) -> None:
    sys.stderr.write(
        "[phase-edit-guard] BLOCK: phase=" + phase
        + " forbids editing source files." + os.linesep
    )
    for t in targets:
        sys.stderr.write("  target: " + t + os.linesep)
    sys.stderr.write(
        "  reason: research/hearing/design phases must not modify code."
        + os.linesep
        + "  bypass: set SKIP_PHASE_GUARD=1 (audited)."
        + os.linesep
    )


def main() -> int:
    if os.environ.get("SKIP_PHASE_GUARD") == "1":
        return 0
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        return 0
    tool_name = payload.get("tool_name", "")
    if tool_name not in EDIT_TOOLS:
        return 0
    tool_input = payload.get("tool_input") or {}
    if not isinstance(tool_input, dict):
        return 0
    targets = [p for p in extract_target(tool_name, tool_input) if is_source(p)]
    if not targets:
        return 0
    root = harness_root()
    if root is None:
        return 0
    phase = current_phase(root)
    if phase is None or phase not in GUARDED_PHASES:
        return 0
    emit_block(phase, targets)
    return 2


if __name__ == "__main__":
    sys.exit(main())
