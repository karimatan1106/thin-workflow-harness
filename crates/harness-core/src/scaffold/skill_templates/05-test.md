# skill: test

このノードのゴール: 単体 / 結合 / E2E / カバレッジを走らせ、改修の影響範囲をカバーする
テストが green であることを根拠付きで報告する。

## 順序

1. **保留 gate の確認** ── このノードに配線されている test gate を見る:
   ```
   harness status
   ```
   `cmd_exit_0` 系がノードに配線されている（テスト gate は blast radius の言語/パッケージ
   から導出される ── Rust+TS なら `cargo nextest && pnpm test` 等、`docs/operations.md`
   §1）。ビルドチェック（`cmd_exit_0 "cargo check --workspace"` 等）が出口 gate /
   `[meta].mandatory_gates` にある場合は、regression test を足したあとでビルドを壊して
   いないかも確認する。

2. **既存 test suite を実行** ── 言語に応じた既定コマンド例:
   ```
   cargo test --workspace           # Rust
   cargo nextest run                # Rust（並列）
   pnpm test                        # Node.js / TypeScript
   pytest                           # Python
   ```
   exit code と failed test を抽出。

3. **テスト失敗時** ── 直接編集で取り繕わない。implement へ戻す:
   ```
   harness back "テスト失敗: <要点>"
   ```
   これ以上進めないなら `harness stuck "<理由>"`。

4. **regression test の追加** ── バグ修正が含まれるなら regression test を 1 本追加する
   （`count_non_decreasing` でテスト総数を縮めないことが縛られている）。plan.md で test
   戦略を立てている場合は対応する test を実装してから再 run。

5. **再 run + 全 pass 確認** ── 影響範囲をカバーするテストが green であること:
   ```
   cargo test --workspace 2>&1 | tail -10
   ```

6. **test 結果 evidence を提出**:
   ```
   harness report-evidence test_result '{"command":"cargo test","exit_code":0,"covered_count":N}'
   ```
   `covered_count` = 影響行/シンボルをカバーしたテスト数。

7. （オプション）テスト結果ログを artifact 登録:
   ```
   harness record-artifact test_report <path> --tag pass
   ```

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "test"` を参照）:

- `cmd_exit_0 <full-suite-cmd>` ── 結合スイートが exit 0 で通る
  （`harness init` で検出された full-suite コマンド、未設定なら
  `false # configure full-suite ...` で必ず fail する ── 埋めること）
- `count_non_decreasing { evidence_key="test_count", baseline_key="test_count_baseline" }`
  ── テスト数が縮んでいない（baseline は最初の run で確立される）
- （あれば）`cmd_exit_0 "<E2E-script>"` / `cmd_exit_0 "cargo llvm-cov --fail-under-lines 95"`
- （あれば）`evidence_recorded test_result` ── 上の `report-evidence` で pass

満たしたら `harness request-transition security`（または `review`、ワークフロー次第）。

## 信頼の源泉

**`cmd_exit_0` 系は harness 自身が再実行するので `test_result` は補助の申告であって
信頼の源泉でない**（`DESIGN.md` §7）── 偽の evidence で gate を通すことはできない。

## 詰まったとき

- ビルドが壊れている → implement へ `harness back "build broken: <要点>"`
- 環境問題（OS 依存・依存欠落）→ `harness ask "<質問>" --option ... --option ...` で
  人間に判断を仰ぐ
- これ以上進めない → `harness stuck "<理由>"`

## 禁止

- test failure を `report-evidence` で pass と偽装すること（gate fail を尊重）
- テストを skip（`#[ignore]` / `it.skip` / `@pytest.mark.skip` 等）で誤魔化すこと
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report_evidence` の `gate` 引数に gate プリミティブ種別名（`count_non_decreasing` 等）
  を渡すこと ── 渡すのは evidence の **key 名**（`test_result` / `test_count` 等）
