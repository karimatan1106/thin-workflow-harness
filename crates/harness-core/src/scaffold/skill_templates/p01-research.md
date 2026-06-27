# skill: preservation / research

このノードのゴール: **挙動保存(rehost/migration)トラックの起点**。oracle は intent でなく **旧システムの実挙動**。
旧の境界・期待 divergence クラス・捕獲計画・等価方針・入力空間を宣言し、人間承認を取る。

★forward の 03-characterize(AC→failing test)を流用するな ── oracle が逆(forward=これから作る intent / preservation=旧から保存)。

## 順序

1. **旧システムの I/O 境界を特定** ── どこで挙動を観測できるか: batch=ファイル入出力(VSAM/順次)・JCL ステップ RC、online(CICS)=トランザクション req/res・3270 画面。Crypto 実証では HTTP エンドポイント/関数 I/O/DB 状態。

2. **期待 divergence クラスを宣言**(`.harness/preservation/input_space.json`) ── 旧の挙動が新環境で分岐しうる軸を partition として列挙。quirk クラス(A エンコード/B COMP-3/C 浮動/D 日付/E 未初期化/F SORT collation/G 採番/H online 並行)のうち該当を**除去不能 seed**として入れる。各 partition = {id, dimension, quirk_class?, description}。**class E/F/G/H は後段 coverage で bare N/A 不可**になるので、捕獲方針(captured_nondeterministic か quarantine)も見積もる。

3. **等価方針を宣言**(`.harness/equivalence.json`) ── 既定は byte/positional exact。容認する差(EBCDIC→ASCII transcode / COMP-3 decimal decode / 浮動 tolerance / 実行日時 volatile)**だけ** per-field opt-in 規則を足す。**各規則に scope(具体 field/picture-range/byte-window)必須・blanket regex 禁止**。各規則に witness:{old,new}(その規則が保存すべき既知隣接バグ)を付けると differential が過剰正規化を自己検出する。★capture では適用しない(raw)。

4. **捕獲計画を立てる**(scale 前提を明文化) ── 1M 行規模では旧系を録画用に走らせるのが binding constraint。**代表 partition members のみ録画**・storage budget・旧系 compute コスト・**cutover 前に録画**(post-cutover は旧系が死ぬ)を計画に記す。

5. **人間承認 + evidence**:
   ```
   harness ask "この捕獲計画で旧を golden 化する? (partition/等価方針/scale)" --option 承認 --option 修正
   harness report-evidence capture_plan '{
     "verdict":"approved_to_capture",
     "partitions":[{"id":"...","quirk_class":"D","description":"..."}],
     "equivalence_rules":["..."],
     "scale_note":"代表 members のみ・cutover 前録画",
     "human_approval":"approved"
   }'
   ```
   partitions が空だと `json_nonempty` で止まる(=入力空間を宣言せよ)。該当無しの真の小変更なら verdict=not_applicable。

6. 進める: `harness advance`

## 完了条件(exit_gates)
- `evidence_recorded capture_plan` + `json_nonempty capture_plan.partitions` + `json_in capture_plan.verdict ∈{approved_to_capture,not_applicable}` + `json_has capture_plan.human_approval == approved`

## 禁止
- forward の intent/spec を oracle にすること(preservation の oracle は旧実挙動)
- 等価規則を blanket(`*`/`.*`/scope 無)で書くこと(保存対象を masking し false-green)
- class E/F/G/H を partition から落とすこと(除去不能 seed)
- 状態ファイル直接編集 / 禁止語(TODO 等)
