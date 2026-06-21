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

## 駆動モードは2つある（重要）

harness には「誰が LLM 生成をやるか」で2つの駆動モードがある:

| モード | 生成の主体 | API を叩くのは | 認証 | Max プランで |
|--------|-----------|--------------|------|-------------|
| **manual-drive（既定・推奨）** | **このセッションの私（公式 Claude Code）** | 私（=正規 Claude Code クライアント） | Max OAuth で正規 | ✅ 使える |
| `harness run`（ApiWorker / 自動運転） | harness binary が呼ぶ別 LLM | **harness.exe 自身** | `ANTHROPIC_API_KEY` or Max OAuth | ✅ 使える（下記） |

**`harness run`（ApiWorker）は API キーでも Max の OAuth でも認証できる。** `auth.rs` の解決順は `ANTHROPIC_API_KEY`（`x-api-key`）→ `CLAUDE_CODE_OAUTH_TOKEN` → `~/.claude/.credentials.json` の `access_token`（`Authorization: Bearer` ＋ `anthropic-beta: oauth-2025-04-20`）。**以前は Max OAuth を harness から流すと公式クライアント検証で即 429 だったが、その検証は撤廃済みで、いまは Max でも動く**（2026-06-22 確認）。どれも無ければ明示エラー。

→ **既定は manual-drive（好みの既定であって強制ではない）。** harness を「ローカルの状態機械（node / gate / artifact 管理）」として使い、生成（調査・設計・実装）は私が直接やると、harness をネットに出さず生成をこのセッションで回せる。自動運転したいときは `harness run`（Max でも可）。

## Usage flow（manual-drive ── 私がワーカー）

1. cd to workspace（隔離 worktree を使うならその worktree へ。`.harness/` は CWD 直下を auto-detect）
2. If `.harness/` missing: `Bash: harness init`
3. `Bash: harness start "<user intent>"` ── run 開始。別 `.harness` を指したい場合だけ `HARNESS_HOME=/path/to/.harness`
4. **per-node ループ（`harness run` は使わない）**。各ノードで以下を私が回す:
   1. `Bash: harness status` ── 現ノード / 出口 gate / reject 残数を確認
   2. `Bash: harness skill` ── 現ノードの skill ファイルの絶対パスを得て **Read で中身を読む**
   3. skill の指示に従い **私が実作業**（調査 / 設計 / 実装 / テスト / レビュー）。生成・編集・コマンド実行はすべて私（Claude Code）が行う
   4. gate 充足:
      - 成果物 gate → `Bash: harness record-artifact <name> <path> [--tag <tag>]`
      - evidence gate → `Bash: harness report-evidence <gate> '<json>'`（または `@file.json`）
   5. `Bash: harness gates` で全 gate が pass か確認 → `Bash: harness advance` で次ノードへ
      - gate 不足なら advance は進まず、不足 gate を表示 → 4 に戻る
      - 設計をやり直すべきと判断したら `Bash: harness back`
5. 人間判断が要る分岐は `Bash: harness ask "<question>" --option A --option B`（私が積む）→ 私が停止してユーザーに提示 → ユーザーが `Bash: harness answer <qid> <choice>`（または私が AskUserQuestion で聞いて代行 answer）
6. 全ノード完走で run 終了。`Bash: harness stats <run-id>` で tool_calls / wall_seconds を確認
7. 詰まったら `Bash: harness status --run <run-id>` / `Bash: harness gates` で state を点検。stale ロックは `Bash: harness run --force-unlock`（これは API を叩かない回収専用）

> 補足: `--run <id>` は status/skill/gates/advance/record-artifact/report-evidence/answer に付けられる。並行 run や run 取り違え防止に明示すると安全。

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
