---
name: thin-workflow-harness
description: workflow.toml + skill-driven workflow runner for LLM agent loops. Use when user has a code change request that benefits from phase-by-phase progression (investigate -> design -> implement -> test -> review). Provides L1-L4 deterministic gates and event-log based state derivation. Skip for one-shot edits or simple Q&A.
---

# thin-workflow-harness

Thin workflow runner for LLM agent loops. Wraps the `harness` CLI binary (Rust).

## When to use

User has a multi-phase code change request, e.g.:
- "investigate X and refactor"
- "design and implement Y feature"
- "review PR for security"

Skip if: one-shot edit, simple Q&A, exploratory questions.

## Prerequisites

- `harness` binary in PATH (`cargo install --path /path/to/thin-workflow-harness/crates/harness`)
- workflow.toml + skills/*.md in workspace `.harness/` (use `harness init` if absent)
- (optional) `harness-lspd` for CKG tool (`cargo install --path /path/to/examples/skill-tools-archive/lsp-daemon`)

## Usage flow

1. cd to workspace
2. If `.harness/` missing: `Bash: harness init`（CWD 直下に `.harness/` をスキャフォールド）
3. `Bash: harness start "<user intent>"` ── CWD/.harness/workflow.toml を auto-detect する。別 workspace を指したい場合だけ `HARNESS_HOME=/path/to/.harness` を明示する。
4. `Bash: harness run --model claude-sonnet-4-6`
5. Monitor stdout/stderr. If questions arise:
   - `Bash: harness questions`
   - User decides answer
   - `Bash: harness answer <qid> <choice>`
6. On success: `Bash: harness stats <run-id>` to review tool_calls / cost
7. On failure: `Bash: harness status --run <run-id>` to inspect state

## Skill repo composition

A skill repo for thin-workflow-harness should contain:
- `.harness/workflow.toml` -- node definitions (skill / exit_gates / next / on_reject)
- `.harness/skills/*.md` -- per-phase LLM instructions
- (optional) `.harness/spec.toml` -- spec validation
- (optional) `bin/` or `tools/` -- tool binaries the skills invoke via `run_command`

## Common patterns

### CKG tool invocation (skill が呼ぶ例)

skill 内で:
```
1. `run_command("harness-lspd find-symbol User --lang rust --root .")`
2. `run_command("harness-lspd refs User --lang rust --root .")`
3. ...
```

LLM が text response を解析して次の行動。

### Gate evaluation

phase 完了時に harness が exit_gates を評価:
- `artifact_registered { name_or_prefix = "report" }` -- record-artifact tool で artifact 登録が必要
- `evidence_provided { key = "test_pass" }` -- report-evidence tool で evidence 登録が必要
- etc

skill 内で「最後に `record-artifact <name> <path>` を呼べ」と instruct。

### Question 機構

skill 内で「曖昧な判断は `ask <question> --option a --option b` で人間判断を求めよ」と instruct。

## Don't

- skill ファイルの中身を harness が解釈すると思わないこと (harness は中身に介入しない)
- harness が CKG tool を提供すると思わないこと (CKG は skill が外部 tool として呼ぶ)
- exit_gates に必要な tool call を skill instruction で漏らさないこと (gate fail -> on_reject ループ)
