# thin-workflow-harness

軽量ワークフローハーネス（thin harness / fat skills / fat code / fat data）。
巨大な既存コードベースへの改修を、十分な上流壁打ちの後、指示通り一発で実装させることを狙う。
状態は append-only イベントログが唯一の真実（LLM の context には持たせない）。
ワークフローはコードでなくデータ（`workflow.toml`）。設計書（`spec.toml`）とコードを決定論的 gate で双方向に同期する。

## ドキュメント

- `DESIGN.md` — 設計の本体（思想・状態モデル・workflow/spec モデル・gate・context 圧縮・topology・並列・人間 touchpoint・skillify・運用上の考慮事項・実装フェーズ計画・オープン論点）
- `docs/schemas.md` — `spec.toml` / `workflow.toml` の確定スキーマ、gate プリミティブ / コマンド / イベント の正典表
- `docs/worker-context.md` — runtime が worker に渡す context バンドルの仕様
- `docs/ckg.md` — コードナレッジグラフの設計
- `docs/skillify.md` — 複数 run またぎの学習・複利（playbook）
- `docs/operations.md` — resilience / セキュリティ / 可観測性 / config 検証 / deliverable ライフサイクル
- `docs/skill-templates.md` — 各ノードの skill 文面の草稿

## 状態

設計ドキュメントは一区切り。実装はこれから（Phase 0 = core lib + テスト + debug CLI から）。
現在の `src/*.rs` は v0 prototype（5 フェーズをハードコードした初期版）── `DESIGN.md` の方向（`workflow.toml` 駆動・プリミティブ gate・worker ランタイム）に作り直す。

## ビルド（v0）

`cargo build --release` / `cargo install --path .`（コマンド名 `harness`）。
`HARNESS_HOME` 環境変数で `skills/` / `state/` の場所を指定（未指定なら CWD 基準）。
