# skill: characterize

このノードのゴール: AC（受入基準）に対応する **failing test** を実装より先に書き、改修の
意図をテストで固定化する。コードの本体には触らない（test 追加のみ）。「テスト先行」phase。

## 前提

- 直前 phase（plan）の `plan` artifact が登録済 ── まずそれを読む

## 順序

1. **plan と spec を読む**
   ```
   harness status                     # 現ノードの保留 gate
   harness spec <F-NNN>               # 該当 F-NNN の AC / invariant
   ```
   artifact の中身を確認:
   ```
   harness artifact plan              # plan の本文を取得
   ```

2. **導出 dispatch(創作でなく上流からの射影)** ── テストは「何をテストするか」を創作せず、上流の
   関数として導出する。読む対象は AC だけでない: spec の**全 [[invariant]] INV-N**、research の
   `interrogation` evidence の **surfaced[]**(在れば=損失/エッジ・レンズタグ STPA-UCA/HAZOP)、
   プロジェクトの**過去バグ再注入ルール**(`.harness/domain_rules.json` が在れば)。固定写像で落とす:
   - **AC 条件 → example test**。ただし出力期待値を**実装を再実行せずオラクル(代数則/メタモルフィック/
     別実装/契約)として言える**なら **property / differential**(INV や「既存式と同値」系 AC はこちら)。
   - **INV / 契約 → property / 契約 assertion**、**損失・エッジ → 境界 / 異常 / 空入力 case**。
   - **多条件 AC は条件ごとに別テストへ分解**。各テストの名前/コメントに
     **`derived_from=<AC-N条件id | INV-N | シナリオtag | domain規則>` を必ず付す**。
   - test は **research で宣言した seam**(`[[acceptance]].seam` の kind/level/locator)で実装する。
     宣言と異なる seam なら `harness back "seam 不一致: ..."`、AC が抽象的なら `harness back "AC が抽象的: ..."`。
   - **mutation が構造的に盲なクラス**(誤った上流ソース・サービス跨ぎ順序・並行性)は source mutation で
     生存を生まないため **RECORDED 実データ fixture の differential / stateful アサーション**で束縛する
     (後段 mutation-dry の母数外)。

3. **既存 test の baseline を取る** ── characterize 開始時点の test 総数を記録:
   ```
   cargo test --workspace 2>&1 | tail -3
   ```
   exit 0 で baseline 確立:
   ```
   harness report-evidence test_count_baseline '{"count": N}'
   ```
   `count_non_decreasing` gate が後段の test phase でこの baseline を参照する。

4. **failing test を追加** ── 各 AC に対応する test を**テストファイルを直接編集して**追加
   （`#[ignore]` で隠さず、実 fail させる ── `edit-file` という harness コマンドは存在しない）。
   追加後の test 数を確認:
   ```
   cargo test --workspace 2>&1 | tail -3
   ```
   想定: test 総数が増え、新規 test は fail する。

   **fixture / 期待値は RECORDED な実データを出所にする** ── 本番 / 実環境の記録（キャプチャした
   API レスポンス・実ログ・実 DB 行・実画面の snapshot）を使い、手書きの合成データで都合よく通る
   期待値を作らない。合成 fixture は実挙動と乖離し、test green が現実を保証しなくなる
   （`NULL / 欠損 / 未購読 / 空` は実データには出るが合成では抜けやすい ── 本番固有バグの温床）。
   実データを記録できないときは `harness ask` で出所を確認する。

   テスト追加後、**spec.toml の対応する `[[acceptance]].test` / `[[invariant]].test` の `"true"` を、
   この赤テストを指す実コマンドへ書き換える**(provenance 束縛スロットの充填＝既存 traceability_closed と
   characterize_gate を実体化)。

5. **characterize_gate を通す(導出元カバレッジ・行カバレッジでない)** ── このノードの exit_gate は
   `node bin/characterize_gate.mjs`(決定論): 全 AC/INV が非"true"の実テストコマンドへ束縛されているかを
   harness 自身で再実行確認する。
   ```
   node bin/characterize_gate.mjs        # 手元確認(cwd=.harness)。CHARACTERIZE_GATE_RUN=1 で赤も実機検証
   ```
   ★この gate は「完了/網羅」を主張しない **floor**。宣言済み上流への束縛のみで、AC/INV 列挙自体の
   完全性は本工程の保証外(列挙漏れは research の deep-grilling/STPA-HAZOP 損失レンズと curated カタログの責務)。
   シナリオは機械全射にせず **signed-empty-set**(blast radius にシナリオ束縛テスト ≥1、または独立評価者
   署名つき「該当シナリオなし」記録。silent skip 禁止)。

6. **進める**:
   ```
   harness advance
   ```

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "characterize"` を参照）:

- `cmd_exit_0 { cmd = "node bin/characterize_gate.mjs" }` ── 全 AC/INV の test スロットが
  非"true"の実コマンドへ束縛(導出元カバレッジの floor。spec 無/AC・INV 無 → N/A で pass)

## 詰まったとき

- AC が抽象的でテストにできない → `harness back "AC-N が抽象的: ..."` で research へ
- coverage tool 未設定 → `harness ask` で人間に判断を仰ぐ
- 進めない → `harness stuck "<理由>"`

## 禁止

- このフェーズでコード本体を編集すること（test 追加のみ。実装は implement で）
- failing test を `#[ignore]` / `.skip()` で隠すこと
- AC を「曖昧なまま」放置すること（テスト化できない AC は AC でない）
- 実データ由来でない都合の良い合成 fixture で AC を満たした体にすること
- **`AC.test` / `INV.test` を `"true"` のまま残すこと**(導出元束縛の floor を空にする)
- **`derived_from` を持たない orphan テストを足すこと**(どの上流も discharge しない創作)
- **特定変異の事後値を直書きする変異過適合テスト**(05 の穴埋めの先取り防止・refactor 連鎖で高手戻り)
- **characterize_gate の緑を「完了/網羅」と読み替えること**(floor であり列挙完全性は別レンズの仕事)
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
