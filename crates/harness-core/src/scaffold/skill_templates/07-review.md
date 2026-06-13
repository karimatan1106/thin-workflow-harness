# skill: review

このノードのゴール: 最終 code review を **2 軸(Standards / Spec)を分離**して行い、両軸を
独立 evidence で `approved` 提出する。**コード正しさに専念する**(マスター設計書は docdesign phase)。
2 軸を分ける理由: 全標準を満たすコードが**誤った feature** を実装し得る/正しい機能が**convention 違反**し得る。
単軸 verdict はこの misalignment を隠す ── 軸を分けて別 gate にすると、一方が緑でも他方で advance が止まる。

## 前提

- test phase 緑(回帰 gate green)、security phase 緑(`security_review` approved)

## 順序

1. **保留 gate を確認**
   ```
   harness status
   ```
   `traceability_closed` + `standards_review`/`spec_review` の approved + `deletion_test` が残っているはず。

2. **トレーサビリティ確認** ── 各 F-NNN について: artifact ≥1・対応する exit 0 test ≥1・orphan なし。
   ```
   harness spec                       # 全 F-NNN
   harness artifact-list              # 登録済 artifact
   ```

3. **diff を全体把握**
   ```
   git diff main..HEAD --stat
   git diff main..HEAD
   ```

4. **Standards 軸**(文書化標準への適合 ── **Spec 軸と混ぜない**)
   `CLAUDE.md` / `.claude/rules/` / `docs/adr/` / `CONTEXT.md` の**文書化された**標準に対して採点する。
   tooling 強制ルール(clippy/lint/fmt)は除外(別途 cmd_exit_0 が見る)。`hard_violation`(文書化標準の明確な違反)と
   `judgment_call`(裁量)を区別:
   - [ ] 命名が intent を表す / 過剰抽象化・premature optimization なし
   - [ ] エラーハンドリング妥当(panic 最小・Result 伝播・`unwrap()` 根拠)
   - [ ] dead code / commented-out なし / 公開 API の doc 最低限 / 過大ファイル(≥200行)なし or rationale

5. **Spec 軸**(発端の要件への整合 ── **Standards 軸と混ぜない**)
   各 F-NNN について `spec.toml` の requirement 文を**引用**し、実装が意図に整合するか判定する。各々 status:
   `met`(完全実装) / `partial`(部分) / `scope_creep`(spec に無い変更) / `missing`(未実装)。
   orphan artifact(どの F-NNN にも紐付かない変更)は **scope_creep 候補**として列挙する。

6. **deletion test**(Ousterhout の depth ヒューリスティック ── 浅いラッパを増やさない)
   新規/改変した各 interface 境界について「その module を**消した**と想像せよ」:
   複雑性が消える=**pass-through(浅い)** / N 個の caller に複雑性が再出現する=**deep(良い抽象)**。
   caller 数は憶測でなく **CKG で実測**する:
   ```
   harness-lspd query impacted-by <sym>     # 消したら壊れる箇所(=caller)を実測
   harness-lspd query closure <sym> --depth 2
   ```
   新設 interface に pass-through(浅い)が混じっていたら指摘する。

7. **lint / format 確認**(任意・推奨): `cargo clippy -- -D warnings` / `cargo fmt --check` / `pnpm lint`。

8. **両軸 + deletion test を evidence で提出**(**マージせず別 key**):
   ```
   harness report-evidence standards_review '{"verdict":"approved","hard_violations":[],"judgment_calls":["naming 改善余地: ..."],"comments":["positive: ..."]}'
   harness report-evidence spec_review '{"verdict":"approved","per_requirement":[{"id":"F-1","quote":"<spec 引用>","status":"met"}],"scope_creep":[]}'
   harness report-evidence deletion_test '{"modules":[{"name":"<sym>","callers_affected":3,"verdict":"deepens"}],"has_shallow_passthrough":false}'
   ```
   どちらかの軸に issue があれば該当軸を `rejected` にして `harness back "<軸>: <理由>"` で implement/plan へ戻す。

9. **次フェーズへ**: `harness request-transition docdesign`

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "review"`）:

- `traceability_closed { }` ── 全 F-NNN に artifact ≥1 と exit 0 test ≥1、orphan なし
- `json_has { evidence_key = "standards_review", json_path = "verdict", eq = "approved" }`
- `json_has { evidence_key = "spec_review", json_path = "verdict", eq = "approved" }` ── **Spec 軸が独立に緑**
- `json_nonempty { evidence_key = "spec_review", json_path = "per_requirement" }` ── 各 F-NNN の整合判定を実体記録
- `evidence_recorded { key = "deletion_test" }` ── 触った module の depth 評価を記録

マスター設計書系 gate(master_design_update / max_lines / spec_refs_exist)は docdesign phase に移動済。

## 詰まったとき

- orphan artifact → 紐付け修正、または `harness back "artifact 紐付け不足"`(Spec 軸 scope_creep)
- test 抜けの F-NNN → `harness back "F-NNN に test 無し"` で characterize へ
- 重大 issue → `harness back "<軸>: <理由>"` で適切な phase に戻す
- 進めない → `harness stuck "<理由>"`

## 禁止

- **2 軸の findings をマージすること**(分離維持が目的 ── 別 evidence key で出す)
- nit-pick だけで rejected(style 議論は別レイヤ)/ comment 0 件で approved(positive を最低 1 件)
- approved を「とりあえず通すため」に書くこと(gate 偽装)
- 状態ファイル(イベントログ)の直接編集
- 禁止語(TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー, 仮置き)を残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名を渡すこと ── 渡すのは evidence の **key 名**
