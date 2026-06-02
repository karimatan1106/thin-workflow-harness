# skill: design-pre (実装前のマスター設計書 更新)

このノードのゴール: **実装に入る前に**、本変更で目指す設計を マスター設計書
(`docs/architecture/` + `docs/adr/`) に**先に**反映し、`design_pre` evidence を提出する。

SDD の核心 ── 「設計を先に書いてから実装する」。plan(spec.toml = この変更専用の要件) の後、
characterize/implement の前に置く。ここで設計の意図を固めることで、実装が設計に従う形になる。
実装後の docdesign は「実装中に判明した気づき」を反映する後段で、本ノードと役割が違う。

## 前提

- plan phase 緑（spec.toml 承認済み、blast radius 確定）
- research で既存設計書を読了済み（master_design_reviewed evidence あり）

## 順序

1. **保留 gate を確認**
   ```
   harness status
   ```
   `evidence_recorded design_pre` ほかが残っているはず。

2. **本変更が目指す設計を判定** ── plan の spec.toml と既存設計書を突き合わせ、
   下のいずれかに該当すれば実装前に設計書を `updated`、該当しなければ `noop`。
   **判定理由を rationale に必ず書く。**
   - 新しいモジュール / 依存関係を導入する → `docs/architecture/02-blocks.md` を先に更新
   - 実行時データフローを変える → `03-runtime.md` を先に更新
   - 品質目標 / SLO / 不変条件を変える → `05-quality.md` を先に更新
   - 新しい設計判断 (Why) を伴う → ADR を**実装前に**起票 (Proposed で起票し、
     実装確定後に docdesign で Accepted に更新してもよい)

3. **設計書を「これから作るもの」として記述** ── 実装はまだ無いので、
   「現状」ではなく「目標とする構成/フロー」を書く。frontmatter の `last-reviewed` を更新。
   ファイル構造・frontmatter・200 行ルール・ADR 運用は docdesign(09) と同一規約に従う。

4. **evidence を提出**

   verdict は **updated / noop の 2 値のみ**(gate `json_in`)。`rationale` は **必須**
   (gate `json_nonempty`)。「実装前に何を設計したか」または「既存設計で足り更新不要な理由」を書く。
   ```
   # 実装前に設計を更新した場合
   harness report-evidence design_pre '{
     "verdict": "updated",
     "rationale": "新 lock 機構を導入するため 02-blocks に RunLock を追加し ADR を Proposed 起票",
     "architecture_sections_changed": ["02-blocks"],
     "adrs_proposed": ["ADR-025-run-lock"]
   }'

   # 既存設計で足りる場合 (小さな変更で設計に影響しない)
   harness report-evidence design_pre '{
     "verdict": "noop",
     "rationale": "既存 02-blocks の構成内に収まる関数追加のみで、新たな設計判断は無いため"
   }'
   ```

5. **次フェーズへ**
   ```
   harness request-transition characterize
   ```

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "design-pre"` を参照）:

- `evidence_recorded { key = "design_pre" }` ── 実装前の設計反映 or noop 宣言が記録済み
- `json_in { evidence_key = "design_pre", json_path = "verdict", one_of = "updated,noop" }`
- `json_nonempty { evidence_key = "design_pre", json_path = "rationale" }`
- `max_lines { path = "docs/architecture/**/*.md", n = 200 }`
- `max_lines { path = "docs/adr/INDEX.md", n = 200, allow_empty = true }`
- `max_lines { path = "docs/adr/ADR-*.md", n = 200, allow_empty = true }`

## 詰まったとき

- 設計判断が割れる → `harness ask` で人間判断 (安易な noop 逃げ禁止)
- 既存設計と矛盾 → ADR supersede 運用 (docdesign 09 の規約参照)
- 進めない → `harness stuck "<理由>"`

## 禁止

- 設計に影響する変更を rationale 薄く `noop` で済ますこと
- `updated` と書いて実際には architecture/ADR を編集しないこと (虚偽 evidence)
- 実装コードをここで書くこと (本ノードは設計書のみ。コードは implement phase)
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名を渡すこと
  ── 渡すのは evidence の **key 名**（`design_pre`）
