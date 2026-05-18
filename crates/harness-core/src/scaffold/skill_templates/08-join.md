# skill: join

このノードのゴール: fork で分岐した並列ブランチを合流させ、合流後の整合性を再検証する。
default workflow には現れない（plan で `fork` / `join` を追加した場合に使う）── plan
ノードの `can_append = true` で workflow.toml に追記して有効化する。

## 用途

- 1 つの requirement を blast radius 互いに素な複数 sub-task に分解した
  （例：`F-007` → `F-007.1` (file A,B) ＋ `F-007.2` (file C,D)）
- 各 sub-task を並列の implement ノードで処理した後、本ノードで合流
- 合流後、全 branch の影響を合わせた状態で再 build / 再 test する

## 前提

- 全 parent ノード（fork の各分岐）が exit 済（gate met）
- workflow.toml に `fork` / `join` がプランナーによって追加されている
  （append-only で `workflow_append_only` gate に違反しないこと）

## 順序

1. **保留 gate を確認**
   ```
   harness status
   ```
   join ノードの exit_gates と、parent からの artifact / evidence を確認。

2. **branch 成果を集約** ── 各 branch が登録した artifact を一覧で確認:
   ```
   harness artifact-list
   ```
   `impl:F-007.1` / `impl:F-007.2` 等が揃っているか。

3. **合流後の build / test を再実行** ── 各 branch 単独では通っても、合流で衝突する
   ことがある:
   ```
   cargo check --workspace
   cargo test --workspace
   ```
   ここで failure があれば、衝突の解決を branch にフィードバック
   （`harness back "<対象 branch>: <衝突要因>"`）。

4. **合流 artifact を登録**（任意・推奨）── 合流後のサマリ:
   ```
   harness record-artifact join_summary <path> --tag merged
   ```

5. **合流 evidence**（任意。workflow.toml で `evidence_recorded { key = "join_ok" }` を
   配線している場合）:
   ```
   harness report-evidence join_ok '{"merged_branches":["F-007.1","F-007.2"],"verdict":"clean"}'
   ```

6. **進める**（join の next ノードへ。通常は test / security / review）:
   ```
   harness request-transition test
   ```

## 完了条件（exit_gates）

`workflow.toml` の `[[node]] id = "join"` で declared した exit_gates に従う。典型例:

- `cmd_exit_0 { cmd = "<test>" }` ── 合流後の test スイートが通る
- `artifact_registered { name_or_prefix = "impl:" }` ── 全 branch の impl artifact が揃う
- （任意）`evidence_recorded { key = "join_ok" }`

## 詰まったとき

- branch 間で merge 衝突（コード / 仕様）→ `harness back "<branch>: <衝突詳細>"` で
  該当 branch の implement に戻す
- 一部 branch が未完 → `harness status` で gate met 確認、未完なら待つか
  `harness ask` で人間判断
- これ以上進めない → `harness stuck "<理由>"`

## 禁止

- branch の成果を握りつぶして上書きすること（artifact append-only）
- 衝突を「とりあえず通すため」のテスト書き換えで隠すこと
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名（`evidence_recorded` 等）を
  渡すこと ── 渡すのは evidence の **key 名**（`join_ok` 等）
