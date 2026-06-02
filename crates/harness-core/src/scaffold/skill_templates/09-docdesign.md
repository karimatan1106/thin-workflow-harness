# skill: docdesign (マスター設計書の作成 / 修正)

このノードのゴール: 本変更を **マスター設計書 (`docs/architecture/` + `docs/adr/`) に反映**し、
`master_design_update` evidence を提出する。workflow の終端 phase。

review (コード正しさ) と分離した専用ノード ── ここは「設計書を実コードと整合させる」
ことだけに専念する。`docs/architecture/` は mutable snapshot、`docs/adr/` は immutable log。

## 前提

- review phase 緑（`review` approved、トレーサビリティ閉鎖済み）

## 順序

1. **保留 gate を確認**
   ```
   harness status
   ```
   `evidence_recorded master_design_update` ほか設計書系 gate が残っているはず。

2. **実装中の気づきを取り込む** ── implement phase が残した `design_note` evidence を必ず読む。
   ここには「design-pre では想定していなかった、実装中に判明した考慮点」が入っている。
   これらをマスター設計書へ反映するのが本ノードの主目的の一つ。
   ```
   harness status                                 # design_note evidence を確認
   ```
   design-pre で書いた設計と、design_note の気づきの差分を埋める形で設計書を更新する。

3. **既存設計書を把握** (research の master_design_reviewed / design-pre と整合)
   ```
   harness outline docs/architecture/README.md   # arc42 全体 ToC
   harness outline docs/adr/INDEX.md              # ADR 一覧 + status
   ```
   無ければ初回 ── この phase で初稿を scaffold する (3-a 参照)。

3. **変更が設計書に影響するか判定** ── 下のいずれかに該当すれば `updated`、
   どれにも該当しなければ `noop` (bug fix / cosmetic 等)。**判定理由を rationale に必ず書く。**
   - モジュール構成 / 依存関係の変更 → `docs/architecture/02-blocks.md` + 該当 `modules/<name>.md`
   - 実行時データフロー / 主要 scenario の変更 → `03-runtime.md`
   - 品質目標 / SLO / 不変条件の変更 → `05-quality.md`
   - scope / actors / 外部 IF の変更 → `01-context.md`
   - 新しい設計判断 (Why) が生じた → ADR 起票 (4 参照)

### 3-a. ファイル構造 (初回なら scaffold、`harness init` の docs skeleton があれば流用)

```
docs/
├── architecture/
│   ├── README.md            # arc42 全体 ToC (≤200 行)
│   ├── 01-context.md        # scope / actors / 外部 IF
│   ├── 02-blocks.md         # module 構成 (Mermaid C4 container 図 必須)
│   ├── 03-runtime.md        # 主要 scenario のデータフロー
│   ├── 04-decisions.md      # ADR への link 表のみ (本文は書かない)
│   ├── 05-quality.md        # 品質目標 / SLO / 不変条件
│   ├── 06-risks.md          # trade-off / 技術的負債
│   └── modules/<name>.md    # 大きい module はサブディレクトリ
└── adr/
    ├── INDEX.md             # ADR 一覧 + status table (≤200 行 / append-only)
    └── ADR-NNN-<slug>.md    # 個別 ADR (≤200 行 / immutable)
```

### 3-b. 全 .md は YAML frontmatter 必須

```yaml
---
doc-id: arch-02-blocks
status: current             # current | superseded | stale
supersedes: []
tags: [architecture, modules, c4-container]
description: 全モジュールの責務・依存関係・Mermaid C4 container 図
last-reviewed: YYYY-MM-DD   # この phase で必ず更新
---
```

## 4. architecture / ADR の更新

**architecture/ 更新** (該当 section のみ touch、200 行超で分割):
- 該当 .md を実コードと整合するよう編集し、`last-reviewed` を更新
- module 構成変更は Mermaid C4 図も同期
- 新規 ADR を起票したら `04-decisions.md` の link 表に append (本文は ADR 側)

**ADR 起票** (新しい Why が生じたときのみ):
- `docs/adr/ADR-NNN-<slug>.md` 新規作成 (NNN = INDEX 末尾 +1、slug = kebab-case)
- frontmatter: `status: Accepted` / `supersedes: []` / `superseded-by: null`
- 本文 5 セクション: Context / Decision / Consequences / Review Trigger / Related
- `INDEX.md` に 1 行 append (`| ADR-NNN | title | Accepted | YYYY-MM-DD |`)
- 既存 ADR を覆す場合: 新 ADR の `supersedes: [ADR-XXX]` + 旧 ADR の
  `superseded-by: ADR-NNN` (旧 ADR は `status: Superseded`、本文は不変)

**STALE マーカ** (drift 可視化): 古くなった section に inline で `[STALE: see ADR-NNN]` を
挿入 (削除しない)。記す ADR は INDEX に存在必須 (orphan 禁止)。

## 5. evidence を提出

verdict は **updated / noop の 2 値のみ** (gate `json_in` が他値を fail)。`rationale` は
**どちらでも必須** (gate `json_nonempty` が空を fail) ── 「なぜ更新したか」または
「なぜ更新不要と判断したか」を必ず言語化する。`no_change` 等の逃げ値・空 rationale で
gate は通せない。

```
# 更新ありの場合 (updated でも rationale 必須)
harness report-evidence master_design_update '{
  "verdict": "updated",
  "rationale": "WS broadcast の coalesce 方針を変更したため 02-blocks と ADR を同期",
  "architecture_sections_changed": ["02-blocks", "modules/ws-server-rs"],
  "adrs_added": ["ADR-024-broadcast-coalesce"],
  "adrs_superseded": ["ADR-018"],
  "stale_markers_added": [],
  "mermaid_diagrams_synced": ["02-blocks.md"]
}'

# no-op の場合 (bug fix / cosmetic 等で構造も Why も変わらない)
harness report-evidence master_design_update '{
  "verdict": "noop",
  "rationale": "fundingRate の ms 揺れ吸収のみで、モジュール構成も設計判断も不変のため"
}'
```

## 6. workflow 終端

このノードの `next = []`。`request-transition` 不要。harness が全 gate met を検知して
done になる (`harness status` で確認)。

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "docdesign"` を参照）:

- `evidence_recorded { key = "master_design_update" }` ── 反映 or noop 宣言が記録済み
- `json_in { evidence_key = "master_design_update", json_path = "verdict", one_of = "updated,noop" }`
  ── verdict は updated/noop のみ（no_change 等の逃げ値を排除）
- `json_nonempty { evidence_key = "master_design_update", json_path = "rationale" }`
  ── updated/noop どちらでも rationale 必須（空文字・未記載は fail）
- `spec_refs_exist { path = "<src glob>" }` ── ソース中の `@spec` 参照先が実在すること
- `max_lines { path = "docs/architecture/**/*.md", n = 200 }` ── 各 .md は 200 行以下
- `max_lines { path = "docs/adr/INDEX.md", n = 200, allow_empty = true }`
- `max_lines { path = "docs/adr/ADR-*.md", n = 200, allow_empty = true }`

## 詰まったとき

- 設計書が肥大 (≥200 行) → `modules/` 配下に分割し元ファイルから link
- 既存 ADR と矛盾する判断 → supersede 運用 (4 参照)、判断が割れるなら `harness ask`
- 反映すべきか判断がつかない → `harness ask` で人間判断を仰ぐ (安易な noop 逃げ禁止)
- 進めない → `harness stuck "<理由>"`

## 禁止

- 設計に影響する変更を rationale 薄く `noop` で済ますこと（gate は通っても設計 drift を生む）
- `updated` と書いて実際には architecture/ADR を一切編集しないこと（虚偽 evidence）
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名を渡すこと
  ── 渡すのは evidence の **key 名**（`master_design_update`）
