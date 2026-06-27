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
- **提示は AskUserQuestion で(既定)** ── `harness ask` でキューした質問は、そのまま AskUserQuestion ツールで
  人間に提示し、選んだ回答を `harness answer <qid> "<選択肢テキスト>"` で記録する(ピッカー UI と harness
  state/audit の両取り)。`harness answer` を省くと state に残らず `no_pending_required_questions` gate が
  未回答で fail する。平文で 1/2 を待つだけにしない。
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

## 深い詰問プロトコル (deep grill-with-docs) ── 上の壁打ちを“設計を壊す問い”で駆動する

目的: 正しいテストの前に **正しい設計** を引き出す。深さは「人が納得するまで」でなく
**「設計を覆す新しい論点が枯れるまで(loop-until-dry)」**。testable(AC/INV/具体例) はこの詰問の **結果**
(step 4-5 に落ちる) ── 詰問が浅いと AC も浅い。設計根拠: thin-workflow-harness `docs/deep-grilling-design.md`。

> 限界(overclaim しない): `正しいテスト ← 正しい設計 ← 深い詰問` は **必要条件で十分でない**
> (深く問うても誤判断しうる/設計が正しくても網羅の穴は残る) → 下流(test/mutation/verify)保険は必須、
> 詰問はその残差を減らす。「枯渇」は“体系的生成を出し切った”であって“設計が健全”の証明ではない。

### A. 深度 triage(設計リスク = 損失への近さ で重さを決める)
OR トリガーのどれか該当 → **deep**、全て否 → **light**:
- **損失に近い**: 許容できない損失(安全/セキュリティ/金/データ整合/不変条件/クリティカルパス)に至る制御に触れる
- **新規性(新Why)**: 新機能/新ポリシー/新相互作用 = 新たな設計判断が要る
- **低検出**: 設計ミスが下流(test/mutation/review)で気づきにくい

迷えば deep(過少詰問 ≫ 過剰詰問)。light 中に隠れリスクが出たら即 deep 昇格。**深さは blast radius(コード量)で決めない**(小×重大を取りこぼす)。FMEA/RPN は使わない。

### B. 独立詰問者に生成させる(deep 時・ADR-059 の前段 = 自己詰問は甘い)
deep のとき下記 C の質問生成は本スレッドでなく **独立サブエージェント**(`Agent`)に委ねる:
- 渡す: intent + 関連 docs(ADR/architecture/CONTEXT/品質目標) + blast radius + C のプロトコル
  + **blast radius 限定の過去インシデント履歴**(専用カタログは作らず既存記録を on-demand 採掘):
  ```
  git log --grep='fix\|再発\|根治\|revert' --oneline -- <blast radius のファイル>   # 触る領域の過去バグ
  ```
  + 該当 ADR(`docs/adr/`)。これで「この変更は失敗クラス○○(この repo 特有の傷跡)を**再来させないか**」を
  生成させる(generic は C が、project 固有の tail はこの履歴採掘が担当 = 直交)。
- 制約: **「想定する実装/答え」を渡さない**(後知恵バイアスを構造的に排除)。「設計を壊す問いを出せ・忖度するな」、
  レンズ/生成器(HAZOP guideword・UCA・過去バグ class 等)タグ付きで **危険順** に返させる。
- 本スレッド: 返った問いを `harness ask`/AskUserQuestion で人間に詰問 → 回答を spec/ADR へ → D の loop。

### C. 生成 = 3つの直交レンズ(単一フレームワークは不完全。要素 × 演算子で相対網羅)
「正しい設計」は1軸でない。各レンズを総当たりし、出た判断を spec へ。docs が薄い/ADR 0件でも効く。
- **レンズ1 損失・失敗(STPA + HAZOP)**: `Losses → Hazards → Unsafe Control Actions(出ない/誤って出る/
  タイミング異常/途中で止まる) → Loss Scenarios(偶発故障 + 敵対動作)`。各入力・制御に HAZOP ガイドワード
  (No/More/Less/Reverse/Early/Late/As-well-as/Part-of)を当てて UCA を炙り出す。セキュリティもここに統合
  (STRIDE/FMEA 不採用)。データ/分散 = ACID + CAP/PACELC + 一貫性モデル + 分散 fallacies。
- **レンズ2 機能・論理の正しさ**: 損失でない「ただ間違う」誤り = 境界/異常/不変条件/property、
  ISO/IEC 25010:2023 機能適合性。← AC/INV の源。
- **レンズ3 設計品質**: Ousterhout(深い/浅いモジュール・結合・複雑性隠蔽)・ドメインモデル整合・抽象/API 一貫性。

新しい Why は ADR draft 起票(前進蓄積 = 詰問が ADR を生む)。3レンズでも絶対網羅は不可能 → 取りこぼしは下流が直交保険。

### D. loop-until-dry(停止条件 = 深さの定義)
C の質問を人間に詰問 → 回答反映 → **独立詰問者を再度回し新論点が出るか確認**。**2周連続で新論点ゼロ = 枯渇 = 停止**。

### E. 記録(深さの担保)
```
harness report-evidence interrogation '{"depth":"deep","triggers":["損失に近い","新規性"],"rounds":2,"sources":["ADR-NNN","過去バグ:<class>","HAZOP:No","STPA:制御が出ない"],"surfaced":["<炙り出した設計穴>"],"verdict":"exhausted"}'
```

## 順序

0. **現実で再現してから spec を書く**（再現が先、spec は後）── 報告された不具合 / 現状挙動を
   実機で再現し、観測した事実（入力 → 現実の出力、本番に近い形 ── NULL / 欠損 / 未購読 / 空 を含む）を
   research_notes に残す。再現できない / 現実と食い違うなら spec を書かず `harness ask` で intent を
   確かめる。再現していない不具合の spec は書かない ── 内部理解だけで spec を起こすと、誤った前提に
   対して下流が「正しく」通過してしまう（verify ノードが最後に外形で観測するが、ここで現実に錨を
   下ろすほど手戻りが安い）。

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
   **blast radius の道具選択 ── コード=CKG / テキスト=grep**: コードシンボルの変更影響は
   `harness-lspd query`(CKG)で引く(位置が返り context が膨らまない)。一方、**文書/設定/skill 等の
   テキスト変更は grep が正当** ── CKG は markdown 散文に解決すべきシンボルを持たず空振りする
   (`symbol not found`)。「シンボルを変えるなら CKG、文言を変えるなら grep」で選ぶ。候補集合を
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
   **AC は内部状態でなく、ユーザー / オペレータが現実の出力で観測できる外形的な結果として書く**
   （例: 画面に X が表示される / API が Y を返す / 通知が届く / ログに Z が出る）。観測できないものは
   AC でない ── 内部関数が値を返すだけの AC は、描画や結合の段で食い違っても green のまま通る。この
   外形 AC が verify ノードの観測対象になる。

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
