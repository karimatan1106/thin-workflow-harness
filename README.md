# thin-workflow-harness

軽量なワークフローハーネス（Rust 実装）。コード変更タスクを 5 フェーズ（research → plan → implement → test → review）に分け、各フェーズの「出口 gate」を決定論的に強制する。

> **設計の最新方針は `DESIGN.md` と `docs/` を参照。現在の `src/*.rs` は v0 prototype（5 フェーズをハードコードした版）であり、`DESIGN.md` の方向（`workflow.toml` / `spec.toml` 駆動・プリミティブ gate・worker ランタイム）に作り直す予定。**
>
> 設計ドキュメント:
> - `DESIGN.md` — 設計方針（16 節）
> - `docs/schemas.md` — `spec.toml` / `workflow.toml` スキーマ・gate プリミティブ・コマンド・イベント種別のリファレンス
> - `docs/worker-context.md` — worker を spawn するときの context 構築仕様
> - `docs/skill-templates.md` — カノニカル skill 文面案（`skills/*.md` の確定文面の案）
> - `docs/ckg.md` — コードナレッジグラフ詳細設計
> - `docs/skillify.md` — 複数 run またぎの学習・複利（playbook / retrospective）

## 思想

- **thin harness / fat skills**: ハーネス本体は状態管理と gate 強制だけ。各フェーズの「何を達成すべきか / どう進めるか」は `skills/*.md` に書く（fat skills）。
- **決定論的な状態管理**: 状態は LLM に持たせない。`state/<run_id>.jsonl`（append-only イベントログ）が唯一の真実。現在状態はイベントをリプレイして導出する純粋関数（`derive_state`）。
- **append-only イベントログ**: 過去のイベントは書き換えない。`reset` も「リセットした」というイベントを追記するだけ（それ以降のイベントだけで再構築する）。
- **in-process gate**: `harness advance` 実行時にその場で gate 関数を評価する。満たさなければ却下イベントを記録して exit 1。
- **LLM は提案するだけ・状態を持たない**: エージェントは成果物を作り artifact / gate evidence を登録するのみ。フェーズを進められるかはハーネスが決める。
- **L1-L4 gate のみ**: gate はすべて決定論的（ファイル存在・行数・JSON フィールド・禁止語など）。LLM の判断を gate にしない。

## ビルド / インストール

```
cargo build --release          # → target/release/harness(.exe)
# または
cargo install --path .         # → ~/.cargo/bin/harness(.exe)。PATH が通っていれば `harness` で実行可
```

依存: `clap` / `serde` / `serde_json` / `chrono`。`Cargo.lock` はコミット対象（binary なので）。

## セットアップ

- 環境変数 `HARNESS_HOME` をこのリポジトリのパスに設定すると、どの作業ディレクトリからでも `harness` が `skills/` と `state/` を見つけられる。未設定なら現在の作業ディレクトリ（CWD）基準。
- 環境変数 `HARNESS_RUN` に run_id を入れておくと `--run` を省略できる。

## 使い方フロー

```
harness start "ログイン処理のリファクタリング"
# -> 表示された run_id を HARNESS_RUN に設定（または以降 --run を渡す）
harness status                 # 現在フェーズ / 読むべき skill のフルパス / 各 gate の pass・fail
# ... skill を読んでから作業 ...
harness record-artifact research_notes notes.md
harness advance                # gate を満たせば次フェーズへ。満たさなければ却下 + 理由
# ... plan / implement / test ...
harness report-gate test_result '{"command":"cargo test","exit_code":0}'
harness advance
harness report-gate review '{"verdict":"approved","notes":"..."}'
harness advance                # review 通過で完了
```

落ちたら `harness back "<理由>"` で 1 つ前のフェーズへ戻る。

### サブコマンド

| コマンド | 役割 |
|---|---|
| `harness start "<intent>"` | 新しい run を開始（run_id = UTC `YYYYMMDD_HHMMSS`、同秒衝突は末尾 `_b` 等） |
| `harness status [--run RUN]` | run_id / intent / 現フェーズ / skill のフルパス / 各 exit gate の pass・fail / artifacts / gate 根拠 / 完了状態 |
| `harness advance [--run RUN]` | 現フェーズの exit gate を全評価。全 pass で次フェーズへ。1 つでも fail なら却下を記録して exit 1 |
| `harness back "<reason>" [--run RUN]` | 1 つ前のフェーズへ戻る |
| `harness record-artifact <name> <path> [--run RUN]` | 成果物ファイルを登録（絶対パスで保存） |
| `harness report-gate <gate> <json\|@file> [--run RUN]` | gate の根拠（JSON）を記録。`@path` でファイルから読む |
| `harness reset [--run RUN] --yes` | run を最初のフェーズに戻す（イベントログに `reset` を追記。`--yes` 必須） |

run_id の解決順: `--run` > 環境変数 `HARNESS_RUN` > `state/` で最終更新が最新の `*.jsonl` の stem。

## フェーズと gate

| # | フェーズ | skill | exit gates |
|---|---|---|---|
| 1 | research | `01-research.md` | `intent_recorded`, `research_notes_recorded` |
| 2 | plan | `02-plan.md` | `plan_artifact_exists`, `plan_artifact_size_ok` |
| 3 | implement | `03-implement.md` | `impl_artifacts_exist`, `impl_artifacts_size_ok`, `no_forbidden_words` |
| 4 | test | `04-test.md` | `test_result_recorded_and_passing` |
| 5 | review | `05-review.md` | `review_recorded` |

| gate | 意味 |
|---|---|
| `intent_recorded` | `start` の intent が非空 |
| `research_notes_recorded` | `research_notes` artifact が実在ファイルかつ中身非空 |
| `plan_artifact_exists` | `plan` artifact が実在ファイルかつ中身非空 |
| `plan_artifact_size_ok` | `plan` の行数 ≤ 200 |
| `impl_artifacts_exist` | `impl:` で始まる artifact が 1 件以上、全て実在ファイル |
| `impl_artifacts_size_ok` | `impl:` 系 artifact が全て行数 ≤ 200 |
| `no_forbidden_words` | `plan` と `impl:` 系のテキストに禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー, 仮置き）が無い |
| `test_result_recorded_and_passing` | `test_result` 根拠が `{"command":String,"exit_code":i64}` 形式で `exit_code == 0` |
| `review_recorded` | `review` 根拠の `verdict` が `"approved"` |

artifact name の規約: research → `research_notes` / plan → `plan` / implement → `impl:<短い識別子>`（複数登録可）。

## 拡張

- フェーズを追加: `src/phases.rs` の `PHASES` に `Phase { name, skill, exit_gates }` を追加し、対応する `skills/*.md` を作る。
- gate を追加: `src/gates.rs` の `eval_gate` の `match` に分岐を足し、どこかのフェーズの `exit_gates` にその名前を入れる。

## LLM / エージェントへ

- まず `harness status` を実行し、表示された **skill ファイルの絶対パスを必ず読んでから**作業する。How はそこに書いてある。
- 状態は `state/<run_id>.jsonl` が唯一の真実。context や別ファイルに状態を持たない。記憶や推測で状態を進めない。
- `harness advance` は exit gate を満たさないと通らない。却下されたら表示された理由を直してから再実行する。
- 成果物に禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー, 仮置き）を残さない。
- フェーズスキップ、状態ファイル（`state/*.jsonl`）の直接編集、テスト失敗を偽って報告することは不可。
