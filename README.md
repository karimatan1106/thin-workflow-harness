# thin-workflow-harness

> **Platform**: Windows only. `compile_error!(not(windows))` で他プラットフォーム build を block しています。Unix 対応は scope 外 (詳細は内部設計判断、`memory/project_thin_workflow_harness.md` 参照)。

> **License**: MIT (see `LICENSE`)

軽量ワークフローハーネス（thin harness / fat skills / fat code / fat data）。
巨大な既存コードベースへの改修を、十分な上流壁打ちの後、指示通り一発で実装させることを狙う。
状態は append-only イベントログが唯一の真実（LLM の context には持たせない）。
ワークフローはコードでなくデータ（`workflow.toml`）。設計書（`spec.toml`）とコードを決定論的 gate で双方向に同期する。

## What this project provides

`harness.exe` ── workflow runner binary。**LLM agent が workflow.toml を駆動するための薄い orchestrator**。

workspace 構成（Phase 5 以降）:

- `crates/harness-core/` ── workflow / state / event log の core lib
- `crates/harness/` ── CLI binary、ApiWorker、runtime

これだけ。CKG/LSP daemon の code は workspace の外。

## What this project does NOT provide

CKG (find-symbol / refs / closure / etc) や LSP daemon は **harness が提供しない**。
これらは **skill が必要に応じて用意するもの**:

- skill repo に tool binary を同梱
- skill repo の README で「rust-analyzer / scip-typescript / 任意 OSS CKG tool を install してね」と user に指示
- 同梱 binary の例: `examples/skill-tools-archive/lsp-daemon/` (本 project が以前提供していた参考実装、現在は維持していない)

skill 側から harness ApiWorker の `run_command` 経由で任意の external tool を呼べる構造になっており、CKG/LSP の choice は skill 作者の自由。

## thin harness, fat skills, fat code, fat data

- harness (this binary) は薄い workflow runner、~1500-2000 行
- skill (.md) が phase の頭脳 + 使う tool 構成を抱える
- code は外部 OSS tool (rust-analyzer / scip / tree-sitter / 任意の wrapper)
- data は append-only event log

## Quick start

```bash
cargo install --path crates/harness
harness init
harness start "..."
harness run
```

## Claude Code skill として登録する

`harness` を Claude Code (Anthropic CLI) から `/thin-workflow-harness` で起動できるようにするには:

```bash
# Linux/macOS:
mkdir -p ~/.claude/skills
cp -r skills/thin-workflow-harness ~/.claude/skills/

# Windows (PowerShell):
mkdir -p $HOME/.claude/skills
cp -r skills/thin-workflow-harness $HOME/.claude/skills/
```

これで Claude Code session 内で `/thin-workflow-harness "<intent>"` で起動できます。

skill の中身: `skills/thin-workflow-harness/SKILL.md` を参照 (description / 使い方 / common patterns)。

## Skill 作者向け

skill repo を作る場合:

1. `workflow.toml` + `skills/*.md` を用意
2. 必要な tool binary を skill repo に同梱（or user に install 指示）
3. skill 内で `run_command("./bin/your-tool find-symbol ...")` 等の instruction を書く

参考: `examples/skill-tools-archive/` は本 project が以前提供していた CKG tool 実装。
skill 作者は fork して skill repo に取り込むか、独自実装を用意する。

## ドキュメント

- `DESIGN.md` — 設計の本体（思想・状態モデル・workflow/spec モデル・gate・context 圧縮・topology・並列・人間 touchpoint・skillify・運用上の考慮事項・実装フェーズ計画・オープン論点）
- `docs/schemas.md` — `spec.toml` / `workflow.toml` の確定スキーマ、gate プリミティブ / コマンド / イベント の正典表
- `docs/worker-context.md` — runtime が worker に渡す context バンドルの仕様
- `docs/ckg.md` — コードナレッジグラフの設計（skill 側責務、参考情報）
- `docs/skillify.md` — 複数 run またぎの学習・複利（playbook）
- `docs/operations.md` — resilience / セキュリティ / 可観測性 / config 検証 / deliverable ライフサイクル
- `docs/skill-templates.md` — 各ノードの skill 文面の草稿
- `docs/onboarding.md` — harness を既存 repo に乗せる手順
- `docs/host-capabilities.md` — 能力と harness の分離（Phase 0↔1）
- `docs/example-walkthrough.md` — 10M 行変更の end-to-end トレース（worked example）
- `docs/failure-modes.md` — 失敗モードカタログ
- `docs/implementation.md` — 実装上の確定事項（言語・クレート・hook 方針・ディレクトリ構成）
- `docs/target-codebase-structure.md` — harness が操作しやすい対象コードベースの理想構造（助言的）

## ビルド

```bash
cargo build --release
cargo install --path crates/harness
```

`HARNESS_HOME` 環境変数で `skills/` / `state/` の場所を指定（未指定なら CWD 基準）。
