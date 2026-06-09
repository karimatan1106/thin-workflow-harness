# skill: review

このノードのゴール: 最終 code review。トレーサビリティ閉鎖（F-NNN ↔ artifact ↔ test）と
code quality を確認し、`review` evidence を `approved` で提出する。
**コード正しさに専念する** ── マスター設計書の作成/修正は次の docdesign phase が担う。

## 前提

- test phase 緑、security phase 緑（`security_review` approved）

## 順序

1. **保留 gate を確認**
   ```
   harness status
   ```
   `traceability_closed` と `json_has review verdict approved` が残っているはず。

2. **トレーサビリティ確認** ── 各 F-NNN について：
   - artifact ≥ 1 件登録済
   - 対応する exit 0 test ≥ 1 件
   - orphan（どの requirement にも紐付かない artifact / test）なし

   ```
   harness spec                       # 全 F-NNN を列挙
   harness artifact-list              # 登録済 artifact 一覧
   ```

3. **diff を全体把握**
   ```
   git diff main..HEAD --stat
   git diff main..HEAD                # 全体に目を通す
   ```

4. **review checklist**

   - [ ] 関数名 / 変数名が intent を表している（short / cryptic でない）
   - [ ] 過剰な抽象化 / premature optimization なし
   - [ ] エラーハンドリングが妥当（panic は最小限、Result で伝播、`unwrap()` の根拠あり）
   - [ ] dead code / commented-out code なし
   - [ ] test カバー：03-characterize の AC 用 test ＋ 既存 test 全 pass
   - [ ] 公開 API の doc コメント（rustdoc / JSDoc / docstring）最低限
   - [ ] 過大ファイル（≥200 行）なし、または rationale あり

5. **lint / format 確認**（任意・推奨）
   ```
   cargo clippy --all-targets -- -D warnings
   cargo fmt --check
   pnpm lint
   ```

6. **review 結果を evidence で提出** ── exit_gate
   `json_has review verdict == "approved"` ＋ `json_nonempty review dimensions` を満たす。
   単一 verdict でなく **採点 rubric (dimensions)** を必ず付ける（L09/L11: 判断の外部化）。
   各次元は level (A〜D) と根拠 note を持つ。最低限 correctness / architecture /
   test_coverage の 3 次元:
   ```
   harness report-evidence review '{"verdict":"approved","dimensions":{"correctness":{"level":"A","note":"全 AC を E2E で確認"},"architecture":{"level":"B","note":"層依存方向 OK・一部 naming 改善余地"},"test_coverage":{"level":"A","note":"F-NNN 全件 test 緑"}},"comments":["positive: ..."]}'
   ```
   issue があるなら `verdict: "rejected"` ＋ `harness back "review issue: ..."` で
   implement や plan に戻す。

7. **次フェーズへ** ── マスター設計書の作成/修正は docdesign phase が担う。
   ```
   harness request-transition docdesign
   ```

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "review"` を参照）:

- `traceability_closed { }` ── 全 F-NNN に artifact ≥1 と exit 0 test ≥1、orphan なし
- `json_has { evidence_key = "review", json_path = "verdict", eq = "approved" }`
- `json_nonempty { evidence_key = "review", json_path = "dimensions" }` ── 採点 rubric 必須

マスター設計書系の gate（master_design_update / max_lines / spec_refs_exist）は
次の docdesign phase に移動した。review はコード正しさに専念する。

## 詰まったとき

- orphan artifact がある → 紐付けを修正、または `harness back "artifact 紐付け不足"`
- test が抜けている F-NNN がある → `harness back "F-NNN に test 無し"` で characterize へ
- review で重大 issue → `harness back "<理由>"` で適切な phase に戻す
- 進めない → `harness stuck "<理由>"`

## 禁止

- nit-pick だけで rejected しないこと（style 議論は別レイヤ）
- comment 0 件で approved （positive feedback も最低 1 件は書く）
- approved を「とりあえず通すため」に書くこと（gate 偽装）
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名（`json_has` 等）を
  渡すこと ── 渡すのは evidence の **key 名**（`review`）
