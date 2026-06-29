---
type: reference
title: "onboarding — harness を既存 repo に乗せる"
description: "> DESIGN.md の補助。設計の方針であって最終確定ではない部分も含む。"
tags: [harness, docs]
---

> DESIGN.md の補助。設計の方針であって最終確定ではない部分も含む。

# onboarding — harness を既存 repo に乗せる

## 1. harness が repo に対して動くのに要るもの

`HARNESS_HOME`（= repo 内 `.harness/`）に以下が要る:

- `workflow.toml` — デフォルトワークフロー。プロジェクトのコマンド（build / test / lint / coverage / フルスイート）を `cmd_exit_0` gate に持つ。
- `[meta]` 設定:
  - `mandatory_gates` — 例: `cmd_exit_0 "cargo check --workspace"`（まだビルドが通ること ── per-crate でなく workspace 全体、`DESIGN.md` §16.1）＋ `cmd_exit_0 "gitleaks detect --no-git --redact"`（ソースに書いたシークレット）。
  - `secrets_glob` — どのファイルにシークレットがあるか。
  - `default_model`。
  - `host` — `"claude-code"` / `"runtime"` 等（→ `docs/host-capabilities.md`）。
- `skills/` — ノード skill。標準8個（research / plan / characterize / implement / test / security / review / join、`docs/skill-templates.md`）＋ Phase 1 用に移植する手順 helper（`security-review.md` / `code-review.md`、`docs/host-capabilities.md`）。プロジェクトで上書き可。
- コード知能バックエンド設定 — Serena/LSP か SCIP（→ `docs/ckg.md`）。
- `state/` — run のイベントログ jsonl・質問キュー・transcripts（git ignore）。

## 2. `harness init` — onboarding コマンド

1. **プロジェクト構造を検出**
   - `Cargo.toml`（Rust、workspace か?）
   - `package.json`（pnpm / npm / yarn? test script? モノレポか — `pnpm-workspace.yaml` / nx / turbo 経由?）
   - `pyproject.toml` / `setup.py`（pytest? poetry / uv?）
   - `go.mod`
   - `pom.xml` / `build.gradle`
   - `Makefile`
   - **CI 設定（`.github/workflows/*.yml`）** — ここに正典の test / build / lint コマンドが書いてあることが多い。パースして学ぶ。
   - → 検出したコマンドで `cmd_exit_0` gate を埋めたドラフト `workflow.toml` を生成。
2. **言語を検出 → コード知能バックエンド選定**
   - Rust → `rust-analyzer scip` または LSP
   - TS → `scip-typescript` または tsserver
   - Python → `scip-python` または pyright
   - …
   - モノレポなら複数。`.harness/ckg.toml` にバックエンドとスコープを宣言。
3. **モノレポのサブツリー検出 → サブツリーごとのコマンドマッピング**
   - 例: `packages/web` → `pnpm --filter web test`。
   - 自動検出が一番難しい。部分検出＋人間の埋め。
4. **`.harness/` をスキャフォールド**
   - `workflow.toml`（ドラフト）
   - `skills/`（デフォルトのコピー、または同梱版へのリンク）
   - `state/`（空＋ `.gitkeep`）
   - `.gitignore`（`state/*.jsonl`・transcripts を ignore）
   - オプションで `.claude/settings.json`（Phase 0 用 phase-guard hook — §9）
5. **`harness validate`** をスキャフォールド config に対して走らせ → エラー列挙。
6. **スモークチェック** — 検出した build / test コマンドの `cmd_exit_0` を試す。本当に exit 0 か? ダメなら検出が間違っている → 人間に flag（「`pnpm test` と推測したが実は `pnpm run test:unit` だった」を捕まえる）。
7. **ドラフトを人間レビューに** — 検出結果（build / test / lint / coverage / フルスイートのコマンド、言語、コード知能バックエンド）を提示 → `harness ask` 風に「合ってる? [confirm / edit]」→ 人間が修正 → `.harness/` にコミット。

## 3. `.harness/` のディレクトリレイアウト

```
.harness/
  workflow.toml       # デフォルトワークフロー（プロジェクトのコマンドを cmd_exit_0 gate に）
  ckg.toml            # コード知能バックエンドの宣言（言語ごと、サブツリーごと）
  skills/             # ノード skill（標準8個 research/plan/characterize/implement/test/security/review/join ＋ 移植手順 helper security-review/code-review、プロジェクト上書き可）
  state/              # run のイベントログ jsonl・質問キュー・transcripts（git ignore）
    .gitkeep
  project-invariants.md   # プロジェクト全体の不変条件（人間が一度書く、§5）
  known-flaky.txt     # flaky テストのリスト（人間が保守）
  playbooks/          # 再発する変更タイプの playbook（時間をかけて積む）
.claude/settings.json  # (オプション) Phase 0 用の phase-guard hook
```

## 4. ブートストラップ問題

harness（Phase 1 ランタイム）自体が Rust プロジェクト。harness を使って harness を開発するには harness が要る → Phase 0（core lib + debug CLI）は「手で」作る（Claude Code または人間）。Phase 1 ができたら自分の repo に `harness init` して以降を harness で開発（dogfooding）。

順:

1. Phase 0 を手で。
2. Phase 1 を手で（Phase 0 の debug CLI で進捗追跡を半補助）。
3. Phase 2+ と継続保守は自分自身に対する harness で。

## 5. 自動検出できない（人間が要る）もの

- **プロジェクト全体の不変条件**（改修ごとではない — それは壁打ち）: 「これは決済システム、カード番号をログに出すな」「公開 API は `domains/*/api/`、migration plan 無しに壊すな」。`.harness/project-invariants.md`、または全改修が継承する base spec の `[[invariant]]`。人間が一度書く。
- `secrets_glob` — どのファイルにシークレットがあるか。ヒューリスティック（`.env` / `*.pem` / `credentials.*`）が拾うが人間がレビュー。
- テストスイートが信用できるか（flaky・既知壊れ）— `.harness/known-flaky.txt`。人間が保守。
- モノレポのサブツリー → コマンドマッピングが検出不完全なとき。
- SCIP precompute の CKG を使うか LSP ライブの CKG か（最速 onboarding は LSP ライブをデフォルト、後で SCIP に格上げ）。

## 6. onboarding の段階（全部要らない、すぐ始められる）

- **Tier 0 — 最小**: `harness init` が build / test / lint コマンドを検出、`cmd_exit_0` gate にした `workflow.toml` をスキャフォールド、CKG 無し（semantic クエリは未提供、または grep フォールバック）。改修は回せる、ただし CKG ベース scope の context 圧縮の恩恵は無い。「Phase 0 レベル onboarding」— 即動く。
- **Tier 1 — コード知能あり**: ＋ CKG バックエンド（LSP ライブまたは SCIP）。scope が精密で安い。「本物」のモード。
- **Tier 2 — フル**: ＋ プロジェクト不変条件、known-flaky リスト、モノレポサブツリーマッピング、パッケージカード（`docs/domains.md`）、再発する変更タイプの playbook。時間をかけて積む。

## 7. 再 onboarding / ドリフト

検出 config は陳腐化しうる（`npm` → `pnpm` 切替、サブツリー追加、test コマンド変更）。

- `harness validate` が*構造的*問題（存在しないコマンドの `cmd_exit_0`）を捕まえる。
- **`harness doctor`** がスモークチェックを再実行し「workflow.toml の build コマンドが clean checkout で exit≠0 — 変わった?」と flag。
- 自動修正はしない（presumptuous）— flag するだけ。

## 8. 雑に構造化された repo への onboarding

harness は動く。ただし:

- blast radius の scope が難しい（CKG の `imports` エッジが絡まり `closure` が大きい集合を返す）→ 壁打ちフェーズがより多く働き、人間の blast-radius 修正も増える。
- `max_lines` gate は `legacy` モード（既存ファイルに ≤200 を強制しない、増やすなだけ — 新規ファイルのみ厳格）。
- 遅いフルスイート gate をより強く頼る。

時間とともに改修が日和見的にファイルを target 構造（`docs/target-codebase-structure.md`）へ抽出していける（「構造へのリファクタ」playbook — 強制ではない）。

## 9. Phase 0 の hook（オプション、ボーナス enforcement）

harness 自身は hook システムを持たない。が Phase 0（agent = Claude Code）では Claude Code の hook を活用できる: `harness init` がオプションで `.claude/settings.json` に PreToolUse hook をスキャフォールド — harness の state を読んで blast radius 外の `Write` / `Edit` を block、`state/<run-id>.jsonl` への書込を block。Phase 1 はインターセプタがやるので不要。詳細は `docs/host-capabilities.md`。
