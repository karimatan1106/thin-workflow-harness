#!/usr/bin/env python3
"""loop-detector.py

Detect tight loops where the agent calls the same tool with the same
input three times in a row. Common failure mode: a worker retries a
broken Edit unchanged after a hook block, or re-runs a flaky test in
hopes it passes this time.

State file: <harness_root>/.harness/loop-state.json
Layout    : { "history": [ {"tool": "...", "hash": "..."}, ... ] }

The history is truncated to the most recent 5 entries.

Exit codes:
  0 -> allow
  2 -> block (3 consecutive matches on the tail)

Skip cases (exit 0):
  * no `.harness/` reachable from cwd (or HARNESS_HOME)
  * stdin is not valid JSON
  * SKIP_LOOP_DETECTION=1
"""

from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path

HISTORY_MAX = 5
LOOP_THRESHOLD = 3
SEP = "|"  # internal separator between tool name and serialized payload


def harness_root() -> Path | None:
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


def load_state(state_path: Path) -> dict:
    if not state_path.exists():
        return {"history": []}
    try:
        data = json.loads(state_path.read_text(encoding="utf-8"))
        if not isinstance(data, dict) or not isinstance(data.get("history"), list):
            return {"history": []}
        return data
    except (OSError, json.JSONDecodeError):
        return {"history": []}


def save_state(state_path: Path, data: dict) -> None:
    try:
        state_path.parent.mkdir(parents=True, exist_ok=True)
        state_path.write_text(
            json.dumps(data, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
    except OSError:
        # Non-fatal: we still let the tool through if we cannot persist.
        pass


def fingerprint(tool_name: str, tool_input) -> str:
    payload = json.dumps(tool_input, sort_keys=True, ensure_ascii=False, default=str)
    blob = (tool_name + SEP + payload).encode("utf-8")
    return hashlib.sha256(blob).hexdigest()


def emit_block(tool_name: str) -> None:
    sys.stderr.write(
        "[loop-detector] BLOCK: tool=" + tool_name
        + " called " + str(LOOP_THRESHOLD)
        + " times with identical input." + os.linesep
        + "  reason: tight loops waste budget and rarely change outcome."
        + os.linesep
        + "  hint  : change the input, fix the underlying issue, or set "
        + "SKIP_LOOP_DETECTION=1." + os.linesep
    )


def main() -> int:
    if os.environ.get("SKIP_LOOP_DETECTION") == "1":
        return 0
    root = harness_root()
    if root is None:
        return 0
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        return 0
    tool_name = payload.get("tool_name", "")
    if not tool_name:
        return 0
    tool_input = payload.get("tool_input")
    digest = fingerprint(tool_name, tool_input)

    state_path = root / ".harness" / "loop-state.json"
    state = load_state(state_path)
    history: list[dict] = state.get("history", [])
    history.append({"tool": tool_name, "hash": digest})
    if len(history) > HISTORY_MAX:
        history = history[-HISTORY_MAX:]
    state["history"] = history
    save_state(state_path, state)

    if len(history) >= LOOP_THRESHOLD:
        tail = history[-LOOP_THRESHOLD:]
        first = tail[0]
        same = all(
            e.get("tool") == first.get("tool")
            and e.get("hash") == first.get("hash")
            for e in tail
        )
        if same:
            emit_block(tool_name)
            return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
