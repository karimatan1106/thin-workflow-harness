---
type: skill
title: "preservation / coverage"
description: "このノードのゴール: preservation/input_space.json の全宣言 partition + quirk クラスが golden で 被覆されているか、"
tags: [skill, workflow]
---

# skill: preservation / coverage

このノードのゴール: `preservation/input_space.json` の全宣言 partition + quirk クラスが golden で **被覆**されているか、
`state/coverage_ledger.json` に独立署名 not_applicable があるか(signed-empty-set)を確認する **floor** ゲート。

★これは「完了/網羅」を主張しない floor。入力空間 partition **列挙自体の完全性は保証外**(誰も宣言しなかった
quirk は不可視=research の deep-grilling と独立評価者の責務=必要条件で十分でない)。薄い taxonomy+寛容な署名は
coverage theater でゲート無しより危険。

## 順序

1. **被覆を確認**:
   ```
   node bin/coverage_gate.mjs
   ```
   各 partition が golden(`golden/manifest.json` の entry.partitions)で覆われているか。覆われていなければ
   golden に当該 quirk 誘発入力を録画(capture へ戻る)するか、`coverage_ledger.json` に独立署名 not_applicable を追記。

2. **★除去不能 class の特則** ── class E(未初期化)/F(SORT collation)/G(採番)/H(online 並行)の seed は
   **bare not_applicable を拒否**される(非決定の最高リスク class が N/A 穴を素通りする false-green を封鎖)。
   これらは `captured_nondeterministic`(golden で admissible-set 捕獲済) か `status:"quarantine"`(独立署名) のみ可。

3. **(任意)構造網羅** ── `COVERAGE_GATE_STRUCTURAL=1` で旧コード構造網羅を `coverage_baseline.json` の ratchet
   (`--update`/`--ratchet`)+ `absorbed_divergences.json` の規則別吸収数 ratchet(新規多数嚥下する規則は再正当化強制)。

4. evidence(終端ノード):
   ```
   harness report-evidence coverage '{"verdict":"adequate","partitions":N,"signed_na":M}'
   ```
   verdict: adequate / gaps_signed(署名で塞いだ) / not_applicable(input_space 無/空)。`next=[]` なので transition 不要。

## 完了条件(exit_gates)
- `cmd_exit_0 node bin/coverage_gate.mjs` + `evidence_recorded coverage` + `json_in coverage.verdict ∈{adequate,gaps_signed,not_applicable}`

## 禁止
- coverage_gate の緑を「入力空間を網羅した」と読み替えること(floor であり列挙完全性は別の責務)
- class E/F/G/H を bare not_applicable で逃がすこと(captured_nondeterministic か署名 quarantine のみ)
- 薄い partition taxonomy で被覆率を演出すること(coverage theater)
- 状態ファイル直接編集 / 禁止語
