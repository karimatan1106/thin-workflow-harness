# skill: research

このノードのゴール: 改修対象を理解し、検証可能な `spec.toml` を作って人間の承認を取る。
コードは編集しない。**壁打ち / scope のループ**であり、唯一「無制限の人間対話が OK」な場所
── ここで over-ask しろ、間違った実装より安い。決まったら即 `spec.toml` に書いて context
から出す（spec は結晶化した壁打ち）。

## サブエージェント隔離（read-heavy な探索）

調査の **読み込み主体の作業**（マスター設計書の pinpoint 読込・CKG による blast radius
探索・コード本体の読込）は `Agent` ツール（Explore / general-purpose）に委譲し、
**蒸留した構造化レポートだけ**を本スレッドに持ち帰る。grep/Read の生出力やシンボル本体で
本スレッドの context を汚さないため（= 長い run でも指示忠実度を保つ）。

- 委譲する: step 2（設計書 pinpoint 読込）と step 3（CKG blast radius）の探索・読込。
  サブエージェントには「対象を読み {関連ファイル, シンボル, 依存, 既存 ADR との矛盾候補,
  blast radius 候補集合} を構造化して返せ。本文は貼らず file:line と要約だけ」と指示する。
- 本スレッドに残す: `harness ask`（人間対話）・`spec.toml` 執筆・`report-evidence` /
  `record-artifact` / `advance`（gate 呼び出し）・全ての判断。
- 返ってきた map を使って spec を書き、research_notes として `record-artifact` する。

## grilling 方式（grill-with-docs）── 壁打ちの規律

- **1 問ずつ** `harness ask`(推奨答えを option 先頭に含める)で詰め、`answer` を待ってから次へ。決定木を順に解く。
- **訊く前にコードを見る** ── コードで答えられる問いは `harness-lspd query` / Read で確認する。
- **曖昧/過負荷の語を即指摘し正典名を提案** ── 解決した語は `CONTEXT.md`(ドメイン用語集、形式は
  `docs/CONTEXT-FORMAT.md`)に **その場で追記**(バッチにしない)。実装詳細は入れない。単一は root の
  `CONTEXT.md`、複数は root の `CONTEXT-MAP.md` 検出。**lazy**(捕捉すべき用語が出た時だけ作る)。
- **具体シナリオでエッジを突く** ── 概念の境界を例で強制的に明らかにする。
- **述べた挙動を実装と照合** ── 矛盾を表に出す。
- **既却下案の確認(`.harness/out-of-scope/`)** ── 着手前にここを見て、同型の要求/アプローチが過去に却下
  されていないか確認(triage 由来)。死蔵案の再調査を避ける。新たに却下が確定したら `.harness/out-of-scope/<slug>.md`
  (却下理由 + ADR/根拠リンク)に記録して残す ── 次回の research が参照する永続メモリ。
- **捨てプロトタイプ可** ── 状態モデル/UI 体裁が紙で詰まらない時は research 内で捨てコードを書いてよい(prototype 由来):
  冒頭に PROTOTYPE/捨てと明記・1コマンド起動・永続化やテストや抽象化はしない。得た**答え(問い+verdict)だけ**を
  spec.toml の rationale か research_notes に蒸留し、足場は spec 確定前に削除(git_clean gate が残骸を咎める)。
- 出口で用語集の状態を evidence に残す:
  ```
  harness report-evidence context_glossary '{"verdict":"created|updated|noop","rationale":"...","terms":["..."]}'
  ```
  `verdict` = `created`(新規作成) / `updated`(追記) / `noop`(ドメイン語彙の更新不要 ── 理由を rationale に必ず書く)。

## 順序

1. **意図の言い直し**（最初、コードを読む前）── 生の intent（`harness status` に出る）を
   自分の言葉で言い直し、人間に確認する:
   ```
   harness ask "この理解で合ってる?（要約: ...）" --option "合ってる" --option "ずれてる"
   ```

2. **既存マスター設計書との照合** ── token-efficient に pinpoint 読込する:

   **2-a. 索引を読む**（frontmatter で relevance 判定、 本文は読まない）
   ```
   Read docs/architecture/README.md                # arc42 全体 ToC (md は Read。CKG outline は code 用)
   Read docs/adr/INDEX.md                          # ADR 一覧 + status
   ```
   無ければスキップ（review で初稿を起こす想定として宣言）。

   **2-b. 関連セクションだけ本文を読む**（arc42 6 セクションのうち本変更に効くもの）
   - F-NNN がモジュール構成に触る → `docs/architecture/02-blocks.md` + `modules/<該当>.md`
   - 実行時挙動に触る → `docs/architecture/03-runtime.md`
   - 品質目標 / SLO に触る → `docs/architecture/05-quality.md`
   - 全体の context に触る → `docs/architecture/01-context.md`

   **2-c. 該当 ADR の Decision/Consequences/Review Trigger を確認**
   ```
   Read docs/adr/ADR-NNN-<slug>.md
   ```
   INDEX.md の link 表から該当 ADR を pinpoint。 関係ない ADR は読まない。

   **2-d. 矛盾候補があれば人間に判断を仰ぐ**:
   ```
   harness ask "既存 ADR-NNN の Decision と矛盾する。 どう扱う?" \
              --option "ADR-NNN supersede 前提で進める (review で新 ADR 起票)" \
              --option "別解で再設計 (research やり直し)" \
              --option "矛盾なし、 誤検知"
   ```

   **2-e. 読了 evidence を残す**:
   ```
   harness report-evidence master_design_reviewed '{
     "verdict": "reviewed",
     "arc42_sections_read": ["02-blocks", "03-runtime"],
     "modules_read": ["modules/ws-server-rs"],
     "adrs_consulted": ["ADR-019", "ADR-022"],
     "conflicts": [],
     "supersede_candidates": []
   }'
   ```
   `verdict` は `"reviewed"`（既存 master あり） / `"absent"`（master 未整備、 review で初稿） /
   `"partial"`（一部のみ存在） のいずれか。

3. **scope（blast radius の発見）** ── CKG (Code Knowledge Graph) tool で候補集合を作る。
   CKG は別バイナリ `harness-lspd`（LSP 経由・多言語）。未導入なら `harness setup-ckg` で
   検出言語の LSP サーバ + harness-lspd を入れる（opt-in）。`harness doctor` が有無を点検する:
   ```
   harness-lspd query outline <file>               # アウトライン（本体ではない）
   harness-lspd query symbol <name>                # シンボル位置
   harness-lspd query closure <sym> --depth N      # 呼び出し下流
   harness-lspd query impacted-by <sym>            # 署名変更時の上流影響
   harness-lspd query refs <sym>                   # 参照箇所（本体は file:line を Read で読む）
   ```
   grep は補助（位置でなくテキストが返り context が膨らむため、構造クエリを優先）。候補集合を
   `requirement.files` のドラフトにして人間に確認:
   ```
   harness ask "blast radius はこれで漏れは?" --option "OK" --option "漏れあり"
   ```

4. **不変条件の特定**（何を壊しちゃダメか）── 各々 `[[invariant]]`（INV-N）として
   `spec.toml` に書き、それぞれに `test` を紐づける。不確かなら `harness ask` で訊く。

5. **受入基準 + test seam 宣言** ── 各々 `[[acceptance]]`（AC-N、`requirement` に紐づく F-ID＋`test` 1 つ）。
   「all AC テスト green」≡「意図した変更」になる程度に具体的に書く。さらに各 AC に **検証 seam を実装前に宣言**
   する(to-prd 由来): `seam = { kind = "existing|new", level = "e2e|integration|unit", locator = "<既存テスト/モジュール名>" }`。
   **既存 seam を優先し、可能な限り高い(external behavior に近い)レベル**を選ぶ。新 seam が要るなら最高レベルで提案し
   `harness ask` で確認。file path でなくシンボル/モジュール名で書く。曖昧な AC は smell（テスト化できない AC は AC でない）。

6. **残った曖昧さ** ── `??` で `spec.toml` 本文に書き、`harness ask` で潰す。
   **決定を訊け、情報を訊くな** ── コードで分かることは自分で見つけよ、人間に訊くのは
   判断だけ。`open_questions_zero` gate は `??` が無くなるまで fail。

7. **最終承認** ── spec 全体を提示して人間 sign-off:
   ```
   harness ask "この spec、欲しい変更か?（要約: F-NNN/AC/不変条件/blast radius）" \
              --option "承認" --option "修正が要る"
   ```
   承認が返ったら evidence を提出:
   ```
   harness report-evidence human_approval '{"verdict":"approved","rationale":"..."}'
   ```

8. 調査メモを artifact 登録（semantic クエリで得た blast radius・依存・判断根拠の蒸留）:
   ```
   harness record-artifact research_notes <path>
   ```

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "research"` を参照）:

- `open_questions_zero` ── 未解決点ゼロ、`??` なし
- `no_pending_required_questions` ── 保留中の `harness ask` がない
- `json_has human_approval verdict == "approved"` ── 上の `report-evidence` で pass
- `evidence_recorded master_design_reviewed` ── 既存マスター設計書の読了/不在宣言が
  記録済み（step 2 の `report-evidence`）
- `evidence_recorded context_glossary` + `json_in verdict ∈ created/updated/noop` +
  `json_nonempty rationale` ── 用語集(CONTEXT.md)の grilling 結果が記録済み（上の grilling 方式）
- （あれば）`blast_radius_declared` ── 各 F-NNN に `files` ≥1

満たしたら `harness advance`。却下 (`advance_rejected`) されたら
`failed_gates` の理由を読んで直し、もう一度 `advance`。

## 詰まったとき

これ以上進めない（gate が満たせない・前提が崩れている・情報が足りない）と判断したら、
`harness advance` を空打ちで繰り返さず正直にエスカレせよ:
```
harness stuck "<理由>"
```
harness が `node_aborted` を書いて人間に回す。

人間に回す/別セッションへ引き継ぐ時は event log 任せにせず **actionable な digest** を残す(handoff 由来):
未了 blocker・保留中の決定・next steps・次に goto すべきノード ID を 1 行ずつ。artifact は再掲せずパス参照、機密は除去。

## 禁止

- このフェーズでコードを編集すること（research のみ）
- 状態ファイル（イベントログ・他人が書く `spec.toml` 箇所）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report_evidence` の `gate` 引数に gate プリミティブ種別名（`evidence_recorded` 等）を
  渡すこと ── 渡すのは evidence の **key 名**（`human_approval` 等）
