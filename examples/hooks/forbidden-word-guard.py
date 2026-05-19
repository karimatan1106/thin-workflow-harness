#!/usr/bin/env python3
"""forbidden-word-guard.py

Block Edit/Write/MultiEdit operations whose new_string / content contains
forbidden placeholder vocabulary. Such words leak into downstream phases
and become silently load-bearing -- e.g. an LLM treats `TODO` as a real
field name in a later refactor. Blocking at PreToolUse keeps the audit
trail clean.

Forbidden vocabulary:
  EN: TODO, TBD, WIP, FIXME, XXX, HACK
  JA: 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー, 仮置き

Detection runs against comment-like lines too (//, #, /* ... */) -- the
goal is to keep placeholders out of the file entirely.

Exit codes:
  0 -> allow
  2 -> block (stderr lists each (file, line, word))

Bypass:
  SKIP_FORBIDDEN_WORDS=1
"""

from __future__ import annotations

import json
import os
import re
import sys

EDIT_TOOLS = {"Edit", "Write", "MultiEdit"}

EN_WORDS = ["TODO", "TBD", "WIP", "FIXME", "XXX", "HACK"]
JA_WORDS = [
    "未定",
    "未確定",
    "要検討",
    "検討中",
    "対応予定",
    "サンプル",
    "ダミー",
    "仮置き",
]

# EN words must be whole-word to avoid hitting `todoList` or `wipeFile`.
EN_PATTERN = re.compile(r"\b(" + "|".join(EN_WORDS) + r")\b")
JA_PATTERN = re.compile("(" + "|".join(re.escape(w) for w in JA_WORDS) + ")")


def scan(text: str) -> list[tuple[int, str]]:
    """Return list of (1-based line number, matched word)."""
    hits: list[tuple[int, str]] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        for m in EN_PATTERN.finditer(line):
            hits.append((lineno, m.group(1)))
        for m in JA_PATTERN.finditer(line):
            hits.append((lineno, m.group(1)))
    return hits


def collect_payloads(tool_name: str, tool_input: dict) -> list[tuple[str, str]]:
    """Return [(label, text_to_scan), ...] for the given tool input."""
    out: list[tuple[str, str]] = []
    file_path = tool_input.get("file_path", "<unknown>")
    if tool_name == "Write":
        content = tool_input.get("content")
        if isinstance(content, str):
            out.append((str(file_path) + " [content]", content))
    elif tool_name == "Edit":
        new_string = tool_input.get("new_string")
        if isinstance(new_string, str):
            out.append((str(file_path) + " [new_string]", new_string))
    elif tool_name == "MultiEdit":
        edits = tool_input.get("edits")
        if isinstance(edits, list):
            for idx, e in enumerate(edits):
                if not isinstance(e, dict):
                    continue
                ns = e.get("new_string")
                if isinstance(ns, str):
                    label = str(file_path) + " [edits[" + str(idx) + "].new_string]"
                    out.append((label, ns))
    return out


def emit_block(findings: list[tuple[str, int, str]]) -> None:
    sys.stderr.write(
        "[forbidden-word-guard] BLOCK: placeholder vocabulary detected."
        + os.linesep
    )
    for label, lineno, word in findings:
        sys.stderr.write(
            "  " + label + ":" + str(lineno) + "  word=" + word + os.linesep
        )
    sys.stderr.write(
        "  reason: forbidden placeholders become load-bearing downstream."
        + os.linesep
        + "  bypass: set SKIP_FORBIDDEN_WORDS=1 (audited)."
        + os.linesep
    )


def main() -> int:
    if os.environ.get("SKIP_FORBIDDEN_WORDS") == "1":
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

    findings: list[tuple[str, int, str]] = []
    for label, text in collect_payloads(tool_name, tool_input):
        for lineno, word in scan(text):
            findings.append((label, lineno, word))

    if not findings:
        return 0
    emit_block(findings)
    return 2


if __name__ == "__main__":
    sys.exit(main())
