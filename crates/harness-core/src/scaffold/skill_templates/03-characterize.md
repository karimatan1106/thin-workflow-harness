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

2. **AC ↔ test の対応を確定** ── 各 AC-N に対応する test 1 本を、**research で宣言した seam**
   (`[[acceptance]].seam` の kind/level/locator、to-prd 由来)で実装する。宣言と異なる seam で書きたく
   なったら spec が不完全 ── `harness back "seam 不一致: ..."` で research に戻す。AC が
   テスト化できない（具体性不足）なら `harness back "AC が抽象的: ..."` で research に戻す。

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

5. **coverage gate** ── このノードの exit_gate `cmd_exit_0 <coverage>` が pass する
   coverage コマンドを走らせる（detect で導出された coverage cmd、未設定なら
   `false # configure coverage ...` が fail を強制）:
   ```
   cargo llvm-cov --workspace            # 例
   pnpm test:coverage                    # 例
   ```

6. **進める**:
   ```
   harness advance
   ```

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "characterize"` を参照）:

- `cmd_exit_0 { cmd = "<coverage>" }` ── coverage コマンドが exit 0
  （未検出のとき `false # configure coverage ...` で必ず fail ── 埋めること）

## 詰まったとき

- AC が抽象的でテストにできない → `harness back "AC-N が抽象的: ..."` で research へ
- coverage tool 未設定 → `harness ask` で人間に判断を仰ぐ
- 進めない → `harness stuck "<理由>"`

## 禁止

- このフェーズでコード本体を編集すること（test 追加のみ。実装は implement で）
- failing test を `#[ignore]` / `.skip()` で隠すこと
- AC を「曖昧なまま」放置すること（テスト化できない AC は AC でない）
- 実データ由来でない都合の良い合成 fixture で AC を満たした体にすること
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
