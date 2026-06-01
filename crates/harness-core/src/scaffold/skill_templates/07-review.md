# skill: review

このノードのゴール: 最終 code review。トレーサビリティ閉鎖（F-NNN ↔ artifact ↔ test）と
code quality を確認し、`review` evidence を `approved` で提出する。workflow の終端 phase。

## 前提

- test phase 緑、security phase 緑（`security_review` approved）

## 順序

1. **保留 gate を確認**
   ```
   harness status
   ```
   `traceability_closed` と `json_has review verdict approved` が残っているはず。

2. **トレーサビリティ確認** ── 各 F-NNN について：
   - artifact ≥ 1 件登録済
   - 対応する exit 0 test ≥ 1 件
   - orphan（どの requirement にも紐付かない artifact / test）なし

   ```
   harness spec                       # 全 F-NNN を列挙
   harness artifact-list              # 登録済 artifact 一覧
   ```

3. **diff を全体把握**
   ```
   git diff main..HEAD --stat
   git diff main..HEAD                # 全体に目を通す
   ```

4. **review checklist**

   - [ ] 関数名 / 変数名が intent を表している（short / cryptic でない）
   - [ ] 過剰な抽象化 / premature optimization なし
   - [ ] エラーハンドリングが妥当（panic は最小限、Result で伝播、`unwrap()` の根拠あり）
   - [ ] dead code / commented-out code なし
   - [ ] test カバー：03-characterize の AC 用 test ＋ 既存 test 全 pass
   - [ ] 公開 API の doc コメント（rustdoc / JSDoc / docstring）最低限
   - [ ] 過大ファイル（≥200 行）なし、または rationale あり

5. **lint / format 確認**（任意・推奨）
   ```
   cargo clippy --all-targets -- -D warnings
   cargo fmt --check
   pnpm lint
   ```

6. **マスター設計書への反映** ── 本変更が「全体図」や「Why の履歴」に影響するなら
   実コードと整合するように更新する。 ディレクトリ構造は `docs/architecture/` (mutable
   snapshot) と `docs/adr/` (immutable log) の 2 系統で運用する。

   **6-a. ファイル構造 (初回 review なら scaffold する)**
   ```
   docs/
   ├── architecture/
   │   ├── README.md            # arc42 全体 ToC (≤200 行)
   │   ├── 01-context.md        # arc42 §1: scope / actors / 外部 IF
   │   ├── 02-blocks.md         # arc42 §2: module 構成 (Mermaid C4 container 図 必須)
   │   ├── 03-runtime.md        # arc42 §3: 主要 scenario のデータフロー
   │   ├── 04-decisions.md      # arc42 §4: ADR への link 表のみ (本文書かない)
   │   ├── 05-quality.md        # arc42 §5: 品質目標 / SLO / 不変条件
   │   ├── 06-risks.md          # arc42 §6: trade-off / 技術的負債
   │   └── modules/             # 大きい module はサブディレクトリ可
   │       └── <name>.md
   └── adr/
       ├── INDEX.md             # ADR 一覧 + status table (≤200 行 / append-only)
       └── ADR-NNN-<slug>.md    # 個別 ADR (≤200 行 / immutable)
   ```

   **6-b. 全 .md は YAML frontmatter 必須**
   ```yaml
   ---
   doc-id: arch-02-blocks
   status: current             # current | superseded | stale
   supersedes: []
   tags: [architecture, modules, c4-container]
   description: 全モジュールの責務・依存関係・Mermaid C4 container 図
   last-reviewed: YYYY-MM-DD   # この review で必ず更新
   ---
   ```

   **6-c. architecture/ 更新の判断基準** (該当する section のみ touch、 200 行を超えたら分割)
   - モジュール構成変更 → `02-blocks.md` + 該当 `modules/<name>.md` + Mermaid C4 同期
   - 実行時データフロー変更 → `03-runtime.md`
   - 品質目標 / SLO / 不変条件変更 → `05-quality.md`
   - 新規 ADR を起票したら `04-decisions.md` の link 表に append (本文は ADR 側)
   - 全 touch ファイルの `last-reviewed` を YYYY-MM-DD に更新

   **6-d. ADR 起票の判断基準** (新しい Why が生じたときのみ)
   - `docs/adr/ADR-NNN-<slug>.md` を新規作成 (NNN は INDEX 末尾 +1、 slug は kebab-case)
   - frontmatter で `status: Accepted` / `supersedes: []` / `superseded-by: null`
   - 本文 5 セクション: Context / Decision / Consequences / Review Trigger / Related
   - `docs/adr/INDEX.md` に 1 行 append (`| ADR-NNN | title | Accepted | YYYY-MM-DD |`)
   - 既存 ADR を覆す場合: 新 ADR の `supersedes: [ADR-XXX]` + 旧 ADR の
     `superseded-by: ADR-NNN` (旧 ADR の `status: Superseded`、 本文は編集しない)

   **6-e. STALE マーカ運用** (drift を可視化)
   - 古くなった section に inline で `[STALE: see ADR-NNN]` を挿入 (削除しない)
   - これにより agent が次回読込時に「ここは ADR-NNN を見ろ」と分かる
   - STALE マーカに記す ADR は INDEX に存在しなければならない (orphan 禁止)

   **6-f. 200 行ルール (harness が gate で強制)**
   - `docs/architecture/**/*.md`、 `docs/adr/INDEX.md`、 `docs/adr/ADR-*.md` の各
     ファイルが ≤200 行であること
   - 超えたら `modules/` 配下にサブディレクトリで分割、 元ファイルからは link

   **6-g. evidence を提出**

   verdict は **updated / noop の 2 値のみ**(gate `json_in` が他値を fail)。`rationale` は
   **どちらでも必須**(gate `json_nonempty` が空を fail) ── 「なぜ更新したか」または
   「なぜ更新不要と判断したか」を必ず言語化する。no_change 等の逃げ値・空 rationale で
   gate を通すことはできない。
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

7. **review 結果を evidence で提出** ── exit_gate
   `json_has review verdict == "approved"` を満たす:
   ```
   harness report-evidence review '{"verdict":"approved","comments":["positive: ..."],"score":"high"}'
   ```
   issue があるなら `verdict: "rejected"` ＋ `harness back "review issue: ..."` で
   implement や plan に戻す。

8. **workflow 終端** ── このノードの `next = []`。`request-transition` 不要。
   完了は harness が `traceability_closed` ＋ `review approved` を検知して自動的に
   出る（または `harness status` で「全 gate met」を確認）。

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "review"` を参照）:

- `traceability_closed { }` ── 全 F-NNN に artifact ≥1 と exit 0 test ≥1、orphan なし
- `json_has { evidence_key = "review", json_path = "verdict", eq = "approved" }`
- `evidence_recorded { key = "master_design_update" }` ── architecture/ADR への
  反映、または no-op 宣言が evidence で記録済み（step 6）
- `json_in { evidence_key = "master_design_update", json_path = "verdict", one_of = "updated,noop" }`
  ── verdict は updated/noop のみ（no_change 等の逃げ値を排除）
- `json_nonempty { evidence_key = "master_design_update", json_path = "rationale" }`
  ── updated/noop どちらでも rationale 必須（空文字・未記載は fail）
- `max_lines { path = "docs/architecture/**/*.md", n = 200 }` ── master の各 .md は
  200 行以下（AI 駆動開発の token-budget 原則）
- `max_lines { path = "docs/adr/INDEX.md", n = 200, allow_empty = true }`
- `max_lines { path = "docs/adr/ADR-*.md", n = 200, allow_empty = true }`

## 詰まったとき

- orphan artifact がある → 紐付けを修正、または `harness back "artifact 紐付け不足"`
- test が抜けている F-NNN がある → `harness back "F-NNN に test 無し"` で characterize へ
- review で重大 issue → `harness back "<理由>"` で適切な phase に戻す
- 進めない → `harness stuck "<理由>"`

## 禁止

- nit-pick だけで rejected しないこと（style 議論は別レイヤ）
- comment 0 件で approved （positive feedback も最低 1 件は書く）
- approved を「とりあえず通すため」に書くこと（gate 偽装）
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名（`json_has` 等）を
  渡すこと ── 渡すのは evidence の **key 名**（`review`）
