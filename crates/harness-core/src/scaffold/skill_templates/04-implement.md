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

2. **実装** ── plan の順序に従って**対象ファイルを直接編集する**（`edit-file` という harness
   コマンドは存在しない ── 編集はエディタ/ツールで直接行う）。
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

6. **設計上の気づきを記録** ── 実装中に判明した「設計書(design-pre)では想定していなかった
   考慮点」を `design_note` evidence に残す。後段の docdesign がこれを必ず参照して
   マスター設計書へ反映する。実装中の気づきを揮発させないための引き継ぎ。
   ```
   # 気づきがある場合 (例: 想定外のエッジケース・新たな不変条件・設計判断の変更)
   harness report-evidence design_note '{
     "notes": [
       "RunLock の stale 判定は PID 生存確認でなく mtime ヒントにした (OS 依存回避)",
       "binance の ms 揺れは funding time 由来で、時境界 floor が必要と判明"
     ]
   }'

   # 気づきが無い場合 (実装が design-pre 通りに収まった)
   harness report-evidence design_note '{"notes": []}'
   ```
   `design_note` は exit_gate で**記録の有無のみ**強制 (空配列も可)。docdesign が中身を読む。

7. **進める**:
   ```
   harness advance
   ```

## デバッグ（diagnose 方式 ── テストが落ちる / バグ修正の時）

当て推量や shotgun fix は禁止。フィードバックループ無しに仮説を立てない。6 フェーズで回す:

1. **フィードバックループを作る**(最重要)── 速く・決定論的・agent が回せる pass/fail 信号を先に確立。
   優先: 失敗テスト > HTTP/curl > CLI+fixture snapshot > headless browser > trace 再生 > 使い捨て harness。
   **harness ではこれは回帰 gate(`regression_gate.mjs`)+ characterize の failing test が既に担う** ──
   まずそれを再現信号にする。2 秒の決定論ループはデバッグの超能力。非決定論バグは 50%+ 再現を狙う。
2. **再現** ── ループを回し、ユーザーの症状と一致(近くの別バグでない)・複数回再現・正確な症状を確認。再現するまで進まない。
3. **仮説**(3〜5 個を **probe 前に** ランク付け)── 各々 **反証可能** に:「X が原因なら Y を変えるとバグが消える /
   Z を変えると悪化する」。リストを `harness ask` で人間に見せると即座に再ランクしてもらえる。
4. **計測(instrument)** ── 各 probe を仮説の予測に対応させ、**1 度に 1 変数** だけ変える。デバッグログには
   **一意プレフィックス `[DEBUG-xxxx]`** を付け 1 grep で消せるように(「全部ログして grep」は避ける)。perf 退行は baseline 測定→bisect(直す前に測る)。
5. **修正 + 回帰テスト** ── 修正の **前に**、バグパターンを **正しい seam(実際の呼び出し箇所)** で突く回帰テストを書く →
   fail を見る → 修正 → pass を見る → 元シナリオでループ再実行。正しい seam が無ければアーキ上の発見として記録。
   **harness では回帰テスト追加は characterize/test の責務 + `--ratchet` で蓄積**。
6. **後始末 + post-mortem** ── 宣言前に: 元の再現が消えた / 回帰テスト pass / **全 `[DEBUG-...]` タグ除去**(出口 gate
   `no_regex` が強制) / 使い捨て prototype 削除 / **真因を commit メッセージに記述**。その後「どんなアーキ変更で防げたか?」を問う。

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "implement"` を参照）:

- `artifact_registered { name_or_prefix = "impl:" }` ── 実装 artifact が `impl:` で始まる
  名前で登録されている
- `cmd_exit_0 { cmd = "<test>" }` ── test スイートが exit 0 で通る
- `evidence_recorded { key = "design_note" }` ── 実装中の設計上の気づき(空配列可)を記録。
  docdesign がこれを参照してマスター設計書へ反映する
- `no_regex { pattern = "[DEBUG-" }` ── diagnose のデバッグタグ(`[DEBUG-xxxx]`)が成果物に残っていない(Phase 6 後始末を強制)
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
