# thin-workflow-harness docs (OKF v0.1)

この `docs/` は **Open Knowledge Format (OKF) v0.1** 準拠の知識バンドル
(<https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf>)。
非予約 `.md` は YAML frontmatter + 非空 `type` を持つ。`index.md`(本ファイル・frontmatter 無)と
`log.md`(変更履歴)は OKF 予約ファイル。概念 ID = パスから `.md` を除いたもの。

## 設計 (design-doc)

- [deep-grilling-design.md](deep-grilling-design.md) — 詰問で正しい設計を引き出す(§9=preservation 専用レンズ)
- [design-writing-design.md](design-writing-design.md) — 設計を「手戻りが出ない形」で書く

## 仕様・内部構造 (reference)

- [schemas.md](schemas.md) — workflow.toml / spec.toml / state スキーマ
- [worker-context.md](worker-context.md) — worker context 構築仕様
- [implementation.md](implementation.md) — 実装上の確定事項と既定
- [host-capabilities.md](host-capabilities.md) — 能力と harness の分離 (Phase 0 ↔ 1)
- [target-codebase-structure.md](target-codebase-structure.md) — 対象コードベースの理想構造
- [ckg.md](ckg.md) — Code Knowledge Graph 詳細設計

## ガイド・運用 (reference)

- [onboarding.md](onboarding.md) — 既存 repo への載せ方
- [operations.md](operations.md) — 運用上の考慮事項
- [example-walkthrough.md](example-walkthrough.md) — end-to-end トレース
- [skillify.md](skillify.md) — run またぎの学習・複利
- [skill-templates.md](skill-templates.md) — カノニカル skill 文面案
- [failure-modes.md](failure-modes.md) — 失敗モードカタログ

## 規約 (OKF v0.1)

- 必須 frontmatter = 非空 `type`(design-doc / reference)。推奨 = title / description / tags / timestamp。
- リンク = 有向・型なしの関係エッジ。安定性重視なら bundle 相対の絶対形 `/...` を推奨(consumer は壊れリンクを許容)。
- 適合チェック = `node .harness/bin/okf_check.mjs`(fail-safe・既定 非ブロッキング・`OKF_STRICT=1` で強制)。
