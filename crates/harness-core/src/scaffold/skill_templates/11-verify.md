# skill: verify

このノードのゴール: 実装が **現実に正しく動くか** を実機で外形観測し、`verify_observation`
evidence に「実際に何を見たか」を構造記録する。標語は **「green != done、観測して done」**。

test ノードまでの white-box gate(型 / unit / 回帰)は「コード ↔ spec の整合」しか保証しない。
だが現実は別の理由で壊れる ── このノードはそれを **外側(ユーザー / オペレータの視点)から実機で
観測** して塞ぐ:

- **描画 / 結合の不一致**: 純関数 unit は通るのに、座標系 / フォーマットの seam がズレて画面に
  出ない・範囲外に落ちる(例: 軸が要求する形と data adapter が返す形の不一致)。
- **本番固有データ**: 合成データのテストは通るが、本番には NULL / 欠損 / 未購読 / 空 が出て
  初めて壊れる。
- **仕様誤読**: 誤った spec に対して正しく通過している(green でも欲しかった結果でない)。

## 順序

1. **何を観測すべきか決める** ── spec の AC(外形的な受入基準)を見る:

   ```
   harness status
   harness skill
   ```

   AC は「画面に X が表示される / API が Y を返す / 通知が届く / ログに Z が出る」のような
   観測可能な外形のはず。それを **実際に発生させて確かめる** のがこのノード。

2. **実機で critical path を動かす** ── 変更の種類に応じて:

   - **UI / 描画変更** → 実ブラウザでアプリを起動し、対象画面を **実際に描画** して見る。
     座標系 / レイアウト / 色 / データが画面に正しく出ているか(範囲外に落ちていないか)。
     スクリーンショットを撮れるなら撮り、視覚で確認する。
   - **backend / API / prod 変更** → サービスを起動するか実エンドポイントを叩き、critical path を
     **本番に近い形のデータ** で実行する。`NULL / 欠損 / 未購読 / 空` を **必ず一度は通す**
     (合成データで隠れる本番固有バグはここでしか出ない)。
   - **本番反映を伴う変更** → 反映後に実エンドポイント / 実画面で、報告された不具合が解消し
     回帰が無いことを観測する。

   自動化できる verify(E2E / screenshot diff / smoke スクリプト)があれば
   `.harness/workflow.toml` の `{verify}` を実コマンドに差し替える(既定は素通り。teeth は下の
   観測 evidence)。

3. **観測した事実を evidence に記録** ── 「通ったはず」でなく **実際に見たもの** を書く:

   ```
   harness report-evidence verify_observation '{
     "verdict": "observed",
     "command": "<起動 / 観測に使ったコマンド or 操作>",
     "observed": "<実際に画面 / 応答 / ログで見たもの。AC と一致したか>",
     "prod_shapes_checked": ["NULL", "欠損", "空"]
   }'
   ```

   - `verdict`: 実機で観測できたら `observed`。観測が原理的に不要(ユーザー可視の挙動を持たない
     純内部 refactor 等)なら `not_applicable`。
   - `observed`: 必須。`observed` なら見た事実、`not_applicable` なら **なぜ観測不要か** の理由を
     書く(空にしない ── ここが gate の teeth)。
   - `prod_shapes_checked`: backend / prod 変更なら実際に通した本番固有の形を列挙
     (UI 変更や N/A は空配列で可)。

4. **食い違いがあれば実装へ差し戻す** ── 観測が AC と違う(画面に出ない / 本番データで壊れる)なら、
   green でも `verdict` を歪めず implement へ戻す:

   ```
   harness back "verify: <観測した現実の食い違い>"
   ```

5. **進める**:

   ```
   harness advance
   ```

## 完了条件(exit_gates)

このノードの出口 gate(`workflow.toml` の `[[node]] id = "verify"` を参照):

- `cmd_exit_0 {verify}` ── 任意の自動 verify コマンド。未設定は素通り(強制力は下の evidence)。
- `evidence_recorded verify_observation` ── 観測記録が登録済み。
- `json_in verify_observation verdict == observed | not_applicable` ── 逃げ値を排除。
- `json_nonempty verify_observation observed` ── 「実際に見たもの / N/A の理由」を必ず言語化。

## 詰まったとき

- 実機を起動できない(環境が無い)→ `harness ask` で人間に観測手段を確かめる。
- 観測したら AC と食い違う → `harness back "verify: ..."` で implement へ。
- これ以上進めない → `harness stuck "<理由>"`。

## 禁止

- 実機で観測せず「通ったはず」で `observed` を記録すること(evidence の偽装)。
- 観測の食い違いを隠して advance すること(green != done)。
- `not_applicable` を理由なしで使うこと(`observed` に「なぜ不要か」を必ず書く)。
- 状態ファイル(イベントログ)の直接編集。
- 禁止語(TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き)を成果物に残すこと。
