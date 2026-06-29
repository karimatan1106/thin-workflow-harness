---
type: reference
title: "実装上の確定事項と既定"
description: "実装上の確定事項と既定。設計の why は DESIGN.md、データ構造のスキーマは docs/schemas.md、ここは「どの言語・どのクレート・どのディレクトリ構成・hook の方針」など実装の地に足のついた決め事。決まってないものは「未決」と明記する。"
tags: [harness, docs]
---

# 実装上の確定事項と既定

実装上の確定事項と既定。設計の why は `DESIGN.md`、データ構造のスキーマは `docs/schemas.md`、ここは「どの言語・どのクレート・どのディレクトリ構成・hook の方針」など実装の地に足のついた決め事。決まってないものは「未決」と明記する。

## 言語・API・フレームワーク（確定）

- **実装言語: Rust**。理由: 新規 script/bot/bridge は Rust 優先の方針（`feedback_prefer_rust`）、v0 prototype も Rust、CPU 効率と単一バイナリ配布。
- **Anthropic API は生 HTTP で直叩き**（Phase 1 のランタイム層）。Rust に公式 Agent SDK が無いため。tool-use ループ（model が tool_use を返す → ツール実行 → tool_result を返す → 繰り返し）・prompt caching・ストリーミング・リトライは自前実装（大した量ではない）。
- **エージェントフレームワークは使わない**。既存の薄い OSS（Pi/OpenClaw、LangChain create_agent 等）に乗る案は検討の上で却下 ── それらは TS/Python であって Rust の方針に反し、`workflow.toml`/`spec.toml`/gate モデルは結局その上に自作になる、外部抽象が「薄い・context 制御」思想と喧嘩する。生 Rust の API クライアント＋tool-use ループ（数百行）を自作する方が摩擦が少ない。「ただし汎用エージェントフレームワークを作らない、この harness が要るループだけ」を守る。

## クレート（既定 ── 争点なし）

- **Phase 0（core lib ＋ debug CLI）**:
  - `clap`（CLI、derive feature）
  - `serde` ＋ `serde_json`（イベントログ jsonl・質問キュー jsonl）
  - `toml`（`workflow.toml`・`spec.toml` のパース ── v0 にはまだ無い、Phase 0 で追加）
  - `chrono`（ISO8601 UTC タイムスタンプ。`default-features=false` + `clock`,`std`）
- **Phase 1（ランタイム層）**: HTTP クライアント。**`ureq`（同期・依存最小）を既定とする** ── tool-use ループは逐次なので async は不要、thin な harness には軽い方が合う（`reqwest` は async・依存が重い）。これは強い確定ではなく「特に理由が無ければ ureq」レベル（要再確認）。
- **Phase 1.5（CKG バックエンド）**: `rusqlite`（CKG の SQLite ストア。bundled feature で SQLite 同梱）。CKG バックエンドの実装方針次第で使う候補 ── `scip`（SCIP `.scip` のパース、SCIP 取り込み経路）／ `tree-sitter` ＋ `tree-sitter-<lang>` ＋ `tree-sitter-stack-graphs`（フォールバック ── 構文構造・`outline`・参照解決）／ `lsp-types`・`lsp-server`・`async-lsp`（LSP / Serena ブリッジ経路）／ `git2` or `git` シェルアウト（増分再索引の変更ファイル取得）。**どれを使うかは CKG バックエンドの実装段階で決まる**（まず Serena / LSP ブリッジ、後で SCIP+SQLite ── `docs/ckg.md` §3・§6.1・§6.2 参照）。SCIP 索引器そのもの（`rust-analyzer scip` 等）は外部プロセスにシェルアウトするので Rust クレートではない。
- 上記以外の重いクレートは入れない方針（thin）。新規クレート追加は「本当に要るか」を都度判断。

## hook の方針（確定）

- **harness 自身は hook/plugin システムを持たない**（anti-thin なので持たせない）。
- **Claude Code 方式の hook（PreToolUse 等）も使わない**。それがやっていた役割（危険コマンドの block、編集制限、フェーズ中の編集禁止）の代替:
  - (a) **ノード出口の決定論的 gate** ── 「research 中に編集するな」のようなリアルタイム block は不要。advance 時に gate が捕まえる（無駄な作業は許すが、間違った状態は確定させない）という設計スタンス（DESIGN §16.1 のトレードオフ）。
  - (b) Phase 1 の **runtime 内 tool-call インターセプタ** ── worker のツール呼び出しを runtime が仲介し、「edit は宣言された blast radius 内」「コマンドは `cmd_allowlist` 内」「作業ディレクトリは worktree」「デフォルト no-network」を強制（DESIGN §10・§16.2、`docs/operations.md` §2）。
- harness の**拡張ポイントはこれだけ**: gate プリミティブ（固定セット ~16 個、`docs/schemas.md` §3）／ `workflow.toml`（データ）／ skills（markdown）／ `cmd_exit_0`（任意の外部スクリプト/テスト/linter/validator を呼ぶ）。プラグイン機構・イベントフック・カスタム gate の動的ロードは**持たない**（足すと太るため）。

## ソースのディレクトリ構成（Phase 0 ── 提案、着手時に最終確定）

- **単一 crate ＋ モジュール分割**（cargo workspace で別 crate に割らない ── thin、workspace は重い）。
- 各ファイル ≤200 行（harness 自身がこのルールを dogfood する）。
- Phase 0 の構成案:

```
Cargo.toml          # deps: clap, serde, serde_json, toml, chrono
src/
  lib.rs            # core lib の公開 API（re-export）
  event.rs          # Event/EventKind 型、jsonl の append/read
  state.rs          # State 型、derive_state（純粋 fold、reset の扱い含む）
  workflow.rs       # Workflow/Node 型、workflow.toml のロードと検証（harness validate の中身）
  spec.rs           # Spec/Requirement/AC/Invariant 型、spec.toml のロード
  gate.rs           # GateResult、eval_gate(name, args, state) ── ~16 プリミティブ（長くなれば gate/ サブモジュールに分割）
  questions.rs      # Question/Answer 型、質問キュー（jsonl）の read/write
  paths.rs          # HARNESS_HOME 解決、run-id 解決
  cli.rs            # debug CLI: argparse、lib 関数への dispatch
  main.rs           # 薄い: cli::run() を呼ぶだけ（#![forbid(unsafe_code)]）
tests/              # 状態機械・gate 評価・config ロード/検証 の結合テスト
```

- **Phase 1 で追加**: `src/runtime/` モジュール（`api.rs` HTTP クライアント＋tool-use ループ ／ `worker.rs` worker spawn とライフサイクル ／ `context.rs` context バンドル構築（`docs/worker-context.md` 準拠）／ `interceptor.rs` tool-call インターセプタ）。`src/main.rs` or 別バイナリで `harness exec/run` モードを足す。Phase 1.5 で `src/ckg.rs`（SQLite ストア＋クエリ、または `src/ckg/`）。── この Phase 1+ の内訳は方針であって最終確定ではない。
- **v0 → Phase 0 のマッピング**: `state.rs` はほぼ流用（イベントログ）／ `gates.rs` はリワーク（ハードコードされた名前付き gate → プリミティブ）／ `phases.rs` は**削除**（`workflow.toml` ロードの `workflow.rs` に置換）／ `spec.rs` は新規 ／ `paths.rs` は流用 ／ `commands.rs`+`main.rs` は新コマンド表面に合わせてリワーク ／ ライブラリ化（`lib.rs`）。

## その他の実装レベル事項

- `Cargo.lock` はコミット対象（バイナリ crate なので）── 既にコミット済み。
- MSRV（最低 Rust バージョン）: 指定しない（最新 stable を前提）。
- lint: `cargo clippy` を通す（v0 で通している）。`#![forbid(unsafe_code)]` を main に置く。warning ゼロを目指す。`cargo build --release` は `lto = true`, `strip = true`。
- CI: 現状なし（必要になったら追加）。
- ライセンス: **未決**。
- `harness validate`（config 検証、`docs/operations.md` §4）は `workflow.rs` / `spec.rs` のロード時バリデーションを呼ぶだけ ── 別実装にしない。

## 未決リスト

- ソースのディレクトリ構成の最終確定（上の案でほぼ良いが着手時に詰める）。Phase 1+ の `src/runtime/` `src/ckg/` の内訳。
- CKG バックエンドの初期実装（Serena / LSP ブリッジ）と後継（SCIP+SQLite）の境界、どの SCIP 索引器をサポートするか（`docs/ckg.md` §6.1）。
- HTTP クライアント `ureq` vs `reqwest`（既定は ureq、要再確認）。
- ライセンス。
- `git-worktree-runner`（外部ツール）との具体的な連携方法（`--worktree` フラグが何を期待するか ── ユーザーが先に worktree を作っておくのか、harness が外部ツールを呼ぶのか）。
