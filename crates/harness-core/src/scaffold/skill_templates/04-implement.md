# skill: implement

このノードのゴール: plan に従ってコードを変更し、03-characterize で書いた failing test を
pass させる。**blast radius（plan で declared した file 集合）の中だけ**を編集する。

## 前提

- plan artifact ＋ ac-list / test が 03 で登録済
- 04 開始時点で「failing test が存在」する状態

## 順序

1. **plan / spec / 失敗 test の確認**
   ```
   harness status                     # 現ノードの保留 gate / 担当 F-NNN
   harness artifact plan              # plan 本文
   harness spec <F-NNN>               # blast radius (files) と AC
   cargo test --workspace 2>&1 | tail -10     # どの test が fail しているか
   ```

2. **実装** ── plan の順序に従って edit_file:
   ```
   harness edit-file <target_file>
   ```
   各変更は spec の `requirement.files` の範囲内に閉じる。範囲外を触りたくなったら
   plan を見直し（`harness back "plan に <file> が無い: ..."`）。

3. **build を通す** ── 各変更後にビルドが壊れていないか:
   ```
   cargo check --workspace
   cargo build --release
   ```
   `[meta].mandatory_gates` の `cmd_exit_0` (check) はここで pass しないと進めない。

4. **failing test が pass する確認** ── exit_gate `cmd_exit_0 <test>` が pass する:
   ```
   cargo test --workspace
   ```
   既存 test の regression があれば原因解析（test 側を書き換えて回避しない）。

5. **実装 artifact を登録** ── exit_gate `artifact_registered { name_or_prefix = "impl:" }`
   が pass するように：
   ```
   harness record-artifact impl:F-NNN <path-to-summary> --tag done
   ```
   `<path>` は実装サマリ（変更ファイル一覧と概要）の小さな md。

6. **進める**:
   ```
   harness request-transition test
   ```

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "implement"` を参照）:

- `artifact_registered { name_or_prefix = "impl:" }` ── 実装 artifact が `impl:` で始まる
  名前で登録されている
- `cmd_exit_0 { cmd = "<test>" }` ── test スイートが exit 0 で通る
- （mandatory_gates 由来）`cmd_exit_0 { cmd = "<check>" }` ── build が通る

## 詰まったとき

- plan の blast radius を超えて編集が必要 → `harness back "plan 不足: <file> が必要"`
- test がどうしても通らない → 実装 strategy を見直す、または `harness back "approach 変更が必要"`
- これ以上進めない → `harness stuck "<理由>"`

## 禁止

- plan / spec で declared していない file を編集すること（blast radius 違反 → gate fail）
- failing test を test 側変更で pass させること（`assert!(true)` 等の偽装）
- test を `#[ignore]` で skip させて pass を主張すること
- 状態ファイル（イベントログ・spec.toml の他人が書く箇所）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名（`artifact_registered` 等）を
  渡すこと ── 渡すのは evidence の **key 名**
