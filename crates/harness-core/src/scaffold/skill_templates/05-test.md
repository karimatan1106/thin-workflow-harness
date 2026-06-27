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

2. **回帰 gate を実機で確認** ── `regression_suites.json` の全スイートを実行し baseline と比較:
   ```
   node bin/regression_gate.mjs            # 全スイート (反復中は --fast / --only <name> で部分実行)
   ```
   個別スイートは config の cmd を直接叩いてもよい（例 `cargo test` / `pnpm exec vitest run` /
   `pytest --color=no -q` / `go test -v ./...`）。落ちた行の pass/fail 差分を見て次へ。

3. **テスト失敗時** ── 直接編集で取り繕わない。implement へ戻す:
   ```
   harness back "テスト失敗: <要点>"
   ```
   これ以上進めないなら `harness stuck "<理由>"`。

4. **生存変異体を導出元にする(creation でなく derivation のループ)** ── 「バグ修正なら 1 本」に
   限らない。step8 の差分 mutation が出す**生存変異体それぞれ**を「この挙動を契約するテストが無い」=
   欠けたテスト仕様として、それを殺すテストを足す。各穴埋めは必ず **AC/INV/シナリオを名指し
   (`derived_from`)**。**特定変異の事後値を直書きする変異過適合テストは禁止**(refactor 連鎖で高手戻り)。
   誤った上流ソース・サービス跨ぎ順序・並行性は source mutation で生存を生まないので
   **RECORDED 実データ fixture の differential / stateful アサーション**で別途束縛(mutation-dry の母数外)。

5. **再 run + 全 pass 確認** ── 影響範囲をカバーするテストが green であること:
   ```
   cargo test --workspace 2>&1 | tail -10
   ```

5b. **E2E を別層として実行（L10・必須 gate）** ── unit は依存を mock するため
   component 境界の欠陥（interface 不一致 / state 伝播 / resource lifecycle /
   環境依存）を構造的に見逃す。これらは E2E でしか出ない。`{e2e}` gate に実 E2E
   コマンド（アプリ起動→クリティカルパス実行）を設定し exit 0 を確認する:
   ```
   <E2E コマンド例: docker compose up + クリティカルパス curl / playwright test>
   ```
   失敗時のエラーメッセージは ERROR（何が失敗）+ WHY（原則）+ FIX（手順）形式で残す。

6. **test 結果 evidence を提出**:
   ```
   harness report-evidence test_result '{"command":"cargo test","exit_code":0,"covered_count":N}'
   ```
   `covered_count` = 影響行/シンボルをカバーしたテスト数。

7. （オプション）テスト結果ログを artifact 登録:
   ```
   harness record-artifact test_report <path> --tag pass
   ```

8. **差分 mutation を「生成ドライバ＋決定論ラチェット」として回す**（teeth・skeptic 修正後）──
   変更行の生存変異体を構造化捕捉し、baseline 比で**悪化させない**ことを機械強制する。
   ```
   node bin/mutate-diff.mjs <base>     # audit(人間可読・非ブロッキング。evidence 用の数字取り)
   node bin/mutate-diff.mjs --gate     # ★決定論ラチェット: baseline 比 新規/退行の非ledger生存ゼロで exit0
   ```
   - **生存変異体 = この挙動を契約するテストが無い**＝欠けたテスト仕様。step4 で **AC/INV 紐付き
     (derived_from)の穴埋め**を足し `--in-diff` scope で再測 →「新規/退行の非ledger生存ゼロ」になるまで反復。
     **zero 生存到達は要求しない**(make-worse 禁止が teeth)。二層: 内側=`--only <suite>`+`--in-diff`、外側=遷移前 full。
   - **equivalent は独立評価者(ADR-059)署名つきで** `state/equivalent_mutants.json` に
     `{"key":"<file::正規化変異テキスト>","evaluator":"independent","reason":"..."}`(行番号不使用)。既存は baseline 吸収。
   - 意図的に baseline を動かすなら `--update` / `--ratchet`(殺せた分だけ除く単調強化)。
   - **JS/TS は要コミット&file→test 写像のため --gate の自動対象外** → プロジェクトの隔離 mutation runner で
     手動測定し evidence/`catalog_waivers.json` に記録(verdict=not_applicable で逃げない)。
   - **catalog**: `node bin/catalog_gate.mjs` が diff の追加行が curated バグ規則(`.harness/domain_rules.json`
     が在れば)の from に触れたら署名付き record を必須化。MUTATE_DOMAIN 注入(from→to)が赤になるテストを書き
     `state/catalog_waivers.json` に `{"rule":"<name>","evaluator":"independent","status":"killed","reason":"..."}`。
   - evidence(人間可読サマリ。teeth は --gate が握る):
   ```
   harness report-evidence mutation_diff '{"verdict":"clean","rust_caught":0,"rust_missed":0,"notes":"ledger/waiver 署名の理由 / N/A 理由"}'
   ```
   verdict は `clean`（新規生存なし）/ `holes_closed`（殺した）/ `not_applicable`（Rust 変更なし・未導入）。
   **`holes_left` は不可**(json_in gate が弾く＝穴を残したまま advance させない)。

## 完了条件（exit_gates）── 回帰 gate（決定論・config 駆動・蓄積）+ 差分 mutation（非ブロッキング）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "test"`）:

- `cmd_exit_0 "node bin/regression_gate.mjs"` ── **harness 自身が再実行する回帰 gate**。
  `regression_suites.json`(SSOT) の全スイートを実機実行し `state/regression_baseline.json` と比較し、
  各スイートで **pass ≥ floor-tol かつ fail ≤ ceiling+tol** を満たさなければ exit≠0 → advance 不可。
  既知の pre-existing 失敗は baseline に織込み済で、**新規失敗（pass 減 / fail 増 / ビルド不能 /
  スイート崩壊）が 1 件でもあれば落ちる**。runner はプリセット(vitest/cargo/jest/pytest/go/dotnet/maven)を
  選ぶか pass/fail スペックを直書きする ── 抽出は config 駆動で harness のコードは言語非依存。
- `cmd_exit_0 <e2e-cmd>` ── **E2E が exit 0（L10・必須）**。unit と層が違う別 gate。未設定なら fail。
- `cmd_exit_0 "node bin/mutate-diff.mjs --gate"` ── **差分 mutation の決定論ラチェット**。baseline 比
  「新規/退行の非ledger生存ゼロ」(make-worse 禁止)で exit0。zero生存到達は要求しない。集合比較で壁時計非依存。
  baseline 空/.rs変更無/cargo-mutants 未導入 → N/A で pass(fail-safe)。
- `cmd_exit_0 "node bin/catalog_gate.mjs"` ── diff が触れた curated バグ規則に署名付き record を必須化(規則無→N/A)。
- `evidence_recorded mutation_diff` ＋ `json_in mutation_diff.verdict ∈ {clean,holes_closed,not_applicable}`
  ── 人間可読サマリを残し **`holes_left` のまま advance させない**。`loop-until-(mutation)-dry` はハード出口でなく目標。

**回帰したら** `harness back "regression: <スイート名と差分>"` で implement に戻して直す。
**baseline を緩めて通すのは禁止**（下の意図的変更を除く）。**スイート構成/期待値を意図的に変えた時のみ** baseline 更新:
- `node bin/regression_gate.mjs --update`  現状値で上書き（再 baseline）
- `node bin/regression_gate.mjs --ratchet`  pass floor 引上げ・fail ceiling 引下げのみ（単調強化=蓄積。旧 `count_non_decreasing` を内包）

満たしたら `harness advance`（または `review`、ワークフロー次第）。

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
