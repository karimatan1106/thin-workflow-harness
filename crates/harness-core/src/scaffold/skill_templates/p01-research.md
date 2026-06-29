# skill: preservation / research

このノードのゴール: **挙動保存(rehost/migration)トラックの起点**。oracle は intent でなく **旧システムの実挙動**。
旧の境界・期待 divergence クラス・捕獲計画・等価方針・入力空間を宣言し、人間承認を取る。

★forward の 03-characterize(AC→failing test)を流用するな ── oracle が逆(forward=これから作る intent / preservation=旧から保存)。

## preservation 詰問(deep-grill)── partition と等価規則を「自己申告」でなく詰問で生成する

A〜H の quirk クラスは**除去不能 seed であって答えではない**。完全性は固定カタログでは出ない(この種の移行の支配的失敗=**omission** は、その repo 固有の tail に宿りカタログのどのクラスにも当たらない)。partition と等価規則は **3つの直交レンズで loop-until-dry に生成**し、deep 変更時は **答え(=どこが壊れるか)を伏せた独立詰問者(別サブエージェント)** に旧 I/O 境界と出力スキーマだけ渡して生成させ、後知恵バイアスを排除する(ADR-059 を設計フェーズに前倒し・forward 01-research と同型。詰問者にはリポジトリの修正済みコードを読ませない=汚染防止)。

- **レンズ1 捕獲漏れ(omission)**:`I/O境界 × ガイドワード{未知入力 / 稀パス(error,reject,timeout,empty,malformed) / 時間依存(年次,月次,うるう,和暦,TZ) / 状態依存(初期DB状態,upsert衝突,実行時のみ到達するデータ) / 環境入力(clock,locale,env,filesystem,charset)}` を総当たり。**加えて、全出力フィールドを「raw passthrough か 派生(計算/分類)か」で必ず仕分ける**。派生フィールドは rewrite で**計算ロジックごと黙って脱落**し既定/空/NULL に落ちる最大の omission リスク → 派生ごとに「旧の計算規則・曖昧時の挙動・分類不能時に書く literal 値」を問う(旧が値を計算していた列を新が NULL 化しても**エラーは出ない**)。
- **レンズ2 オラクル忠実性**:rewrite が保存でなく「直す/整える/正規化する」誘惑はどこか。再現すべき既知 quirk・既定値は何か。**期待値は旧の捕獲出力由来か、それとも合成(synthetic)か** ── 合成 fixture は本番が NULL/空でも pass し omission を隠す(captured-only を強制。witness 隣接バグを各規則に付す)。
- **レンズ3 等価とフィルタ波及(両側の誤り)**:各等価規則は (a)本物の divergence を masking しないか (b)必然差で signal を埋もれさせないか。1:1 再現不能な出力(採番/並行/時刻)は不変条件オラクルへ。**最重要:どの下流 consumer がどの列で filter / join / key するか** ── その列が黙って既定/空/NULL になると consumer の挙動が**エラー無しで**変わる(行が漏れる/消える/alert が止まる)。各 partition にこの「沈黙の波及先」を併記する。
- **深度トリガー(OR)**:`派生フィールドを持つ / 下流が filter・join する列 / 非決定出力 / class E〜H 該当` のいずれかで deep(独立詰問者・loop-until-dry)。明確に trivial な passthrough のみ軽く。
- **停止**:新しい partition / 等価規則 / 沈黙波及が出なくなるまで(loop-until-dry)。固定リスト充足では止めない(弱い詰問は早く枯れる=偽枯渇)。
- **限界(overclaim しない)**:詰問は omission を**減らす**が完全性は証明しない。最終歯止めは下流(differential/coverage/飽和/独立評価者)。

## 順序

1. **旧システムの I/O 境界を特定** ── どこで挙動を観測できるか: batch=ファイル入出力(VSAM/順次)・JCL ステップ RC、online(CICS)=トランザクション req/res・3270 画面。Crypto 実証では HTTP エンドポイント/関数 I/O/DB 状態。

2. **期待 divergence クラスを宣言**(`.harness/preservation/input_space.json`) ── partition は上の**レンズ1 を loop-until-dry に回して生成**する(quirk クラス A エンコード/B COMP-3/C 浮動/D 日付/E 未初期化/F SORT collation/G 採番/H online 並行 は**答えでなく除去不能 seed**として混ぜるだけ)。**まず全出力フィールドを raw / 派生 に仕分け、派生は必ず partition 化**する。各 partition = {id, dimension, quirk_class?, field?, derivation?(派生時の旧計算規則), downstream_filter?(その列で filter/join する下流), description}。**class E/F/G/H は後段 coverage で bare N/A 不可**になるので、捕獲方針(captured_nondeterministic か quarantine)も見積もる。

3. **等価方針を宣言**(`.harness/equivalence.json`) ── 既定は byte/positional exact。容認する差(EBCDIC→ASCII transcode / COMP-3 decimal decode / 浮動 tolerance / 実行日時 volatile)**だけ** per-field opt-in 規則を足す。**各規則に scope(具体 field/picture-range/byte-window)必須・blanket regex 禁止**。各規則を**レンズ3 の両側で検査**:(a)本物の divergence を masking しないか (b)必然差で signal を埋もれさせないか。各規則に witness:{old,new}(その規則が保存すべき既知隣接バグ)を付けると differential が過剰正規化を自己検出する。★capture では適用しない(raw)。**期待値は旧の捕獲出力からのみ作る(合成 fixture 禁止=本番 NULL/空を pass させ omission を隠す)**。

4. **捕獲計画を立てる**(scale 前提を明文化) ── 1M 行規模では旧系を録画用に走らせるのが binding constraint。**代表 partition members のみ録画**・storage budget・旧系 compute コスト・**cutover 前に録画**(post-cutover は旧系が死ぬ)を計画に記す。

5. **人間承認 + evidence**:
   ```
   harness ask "この捕獲計画で旧を golden 化する? (partition/等価方針/scale)" --option 承認 --option 修正
   harness report-evidence capture_plan '{
     "verdict":"approved_to_capture",
     "partitions":[{"id":"...","quirk_class":"D","field":"...","derivation":"旧の計算規則","downstream_filter":"この列でfilterする下流","description":"..."}],
     "equivalence_rules":["..."],
     "interrogation":"loop-until-dry 枯渇・派生/raw 仕分け済・独立詰問者使用(deep時)",
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
- **出力フィールドの raw/派生 仕分けを省くこと**(派生列の計算ロジック脱落が最大の omission・NULL 化してもエラーが出ない)
- **期待値を合成 fixture で作ること**(captured-only。合成は本番 NULL/空を pass させる)
- **下流が filter/join する列の「沈黙の波及先」を partition に併記しないこと**
- 状態ファイル直接編集 / 禁止語(TODO 等)
