---
type: skill
title: "review"
description: "このノードのゴール: 最終 code review を 2 軸(Standards / Spec)を分離して行い、両軸を"
tags: [skill, workflow]
---

# skill: review

このノードのゴール: 最終 code review を **2 軸(Standards / Spec)を分離**して行い、両軸を
独立 evidence で `approved` 提出する。**コード正しさに専念する**(マスター設計書は docdesign phase)。
2 軸を分ける理由: 全標準を満たすコードが**誤った feature** を実装し得る/正しい機能が**convention 違反**し得る。
単軸 verdict はこの misalignment を隠す ── 軸を分けて別 gate にすると、一方が緑でも他方で advance が止まる。

## 独立評価者が verdict を出す（同一 LLM 自己採点の禁止 ── generator/evaluator 分離）

**Standards 軸 / Spec 軸の最終 verdict は、実装を生成した本スレッドが出してはならない。**
新鮮な敵対的サブエージェント（`Agent` ツール）に出させ、本スレッドはその verdict を
**relay するだけ**。書いた本人は自分のコードの採点に甘すぎる（self-preferential bias）。
`traceability_closed` は決定論 gate が握るが、「品質は妥当か / 要件に整合するか」という
2 軸判定は決定論再実行できないため独立評価者で代替する。

**委譲する（評価者サブエージェント）:**

- input: `git diff main..HEAD`、`harness spec`/`artifact-list`、2 軸 checklist、`spec.toml` 要件文
- **これ以外を渡さない（最小コンテキスト）**: 生成者(本スレッド)の推論・弁護・前段の説明・
  「なぜ正しいか/要件を満たすか」の主張を評価者 prompt に**含めるな**。含めると評価者が説得され
  弁明の経路になる ── 評価者は diff と要件だけから独立に判定する。
- 指示（評価者の stance）: 「お前は**敵対的コードレビュアだ。この変更は壊れている/要件を
  取り違えていると仮定**し、正しいと証明されるまで approved を出すな。褒めるな。Standards 軸と
  Spec 軸を**混ぜず別々に**採点。lint/clippy/fmt と `harness-lspd query impacted-by/closure`
  （deletion test の caller 実測）は**自分で実行**して出力を貼れ（読むだけ不可）。各軸とも全項目
  クリアのときだけ verdict=approved、一つでも残れば rejected ＋ 各理由」
- **公平性の本体は fresh context / 別セッション**（共有 context を断てば自己説得の連鎖が消え、
  同一モデルでも公平に判定できる）。**評価者は fork 禁止**（`subagent_type:"fork"` は本スレッドの
  context を継承し自己採点へ逆戻りする ── 非 fork の fresh agent を使う）。別モデルは**系統的な
  共有盲点への追加防御**で必須でない。最良は **review/security を別セッションで cold に回す**
  （`harness status --run <id>` で pickup、または `harness run` worker）── 分離がプロセス事実に
  なり evidence 捏造の余地も消える。
- return（構造化・**2 軸を混ぜない**）: `{standards:{verdict, evaluator_model, observed[],
  hard_violations[], judgment_calls[]}, spec:{verdict, evaluator_model, per_requirement[],
  scope_creep[]}, deletion_test:{modules[], has_shallow_passthrough}}`

**本スレッドに残す（relay と authoring・操舵のみ）:**

- 評価者の各軸 verdict を**そのまま** `report-evidence standards_review`/`spec_review` に載せる。
  **approved への書き換え禁止**（評価者が rejected なら本スレッドで approved にしない）。
- `harness back` / `advance`・人間判断。

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

8. **評価者の両軸 + deletion test を relay**(**マージせず別 key**。
   各軸とも `json_has ... evaluator eq independent` が必須 ── 評価者が返した verdict をそのまま):
   ```
   harness report-evidence standards_review '{"verdict":"approved","evaluator":"independent","evaluator_model":"<別モデル tier>","observed":["cargo clippy -> 0 warnings"],"hard_violations":[],"judgment_calls":["naming 改善余地: ..."],"comments":["positive: ..."]}'
   harness report-evidence spec_review '{"verdict":"approved","evaluator":"independent","evaluator_model":"<別モデル tier>","per_requirement":[{"id":"F-1","quote":"<spec 引用>","status":"met"}],"scope_creep":[]}'
   harness report-evidence deletion_test '{"modules":[{"name":"<sym>","callers_affected":3,"verdict":"deepens"}],"has_shallow_passthrough":false}'
   ```
   評価者がどちらかの軸に issue を残したら該当軸を `rejected` のまま relay し（**本スレッドで
   approved に書き換えず**）`harness back "<軸>: <理由>"` で implement/plan へ戻す。

9. **次フェーズへ**: `harness advance`

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

- **本スレッド（生成者）が最終 verdict を出すこと**（判定は独立した敵対的評価者
  サブエージェントが出す。本スレッドは relay と authoring・操舵のみ）
- **評価者の rejected を本スレッドで approved に格上げすること**（より厳しい側へ倒すのは可）
- **2 軸の findings をマージすること**(分離維持が目的 ── 別 evidence key で出す)
- nit-pick だけで rejected(style 議論は別レイヤ)/ comment 0 件で approved(positive を最低 1 件)
- approved を「とりあえず通すため」に書くこと(gate 偽装)
- 状態ファイル(イベントログ)の直接編集
- 禁止語(TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー, 仮置き)を残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名を渡すこと ── 渡すのは evidence の **key 名**
