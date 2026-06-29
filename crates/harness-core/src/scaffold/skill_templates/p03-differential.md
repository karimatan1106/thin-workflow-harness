---
type: skill
title: "preservation / differential"
description: "このノードのゴール: 同一 input を 旧golden ↔ 新システム へ流し、equivalence.json の per-field 等価で比較。"
tags: [skill, workflow]
---

# skill: preservation / differential

このノードのゴール: 同一 input を **旧golden ↔ 新システム** へ流し、equivalence.json の per-field 等価で比較。
全 divergence を安定アドレス(JSON path)+content-hash 付きで `state/divergences.json` に**列挙**する
(★報告生成が teeth=exit0。divergence の有無で止めない=後段 reconcile が署名裁定で握る)。

## 順序

1. **新系の base を指す** ── 新システムの出力取得先を env で:
   ```
   PRESERVATION_NEW_BASE=<新系base> node bin/differential_gate.mjs
   ```
   golden 無/新系無/差分無 → N/A(exit0)。`state/divergences.json` と `state/absorbed_divergences.json` を生成。

2. **order は positional 既定** ── 配列順は保存対象(class F)。本当に順序非依存な field だけ equivalence.json で
   `order:multiset` を opt-in。SORT/MERGE 出力に multiset を当てるな。

3. **非決定 field は old value-set/tolerance vs new** で比較し false divergence を出さない(capture で分類済の field)。

4. **★過剰正規化の自己検出(per-rule tripwire)** ── equivalence.json の各規則 R に witness:{old,new}
   (R が保存すべき既知隣接バグ)を宣言してあれば、R が witness を green 化したとき differential_gate が exit1。
   赤が出たら R の scope を絞れ(masking しすぎ)。

5. **adapter** ── HTTP は内蔵。file/byte/DB は: 固定長/RDW record framing は byte adapter、
   write-path 状態は `node bin/db-assert.mjs`(read に出ない列 NULL 化等の oracle)。

6. evidence + 進める:
   ```
   harness report-evidence differential '{"verdict":"divergences_enumerated","count":N,"absorbed":{...}}'
   harness advance
   ```
   verdict: equivalent(差0) / divergences_enumerated / not_applicable。

## 完了条件(exit_gates)
- `cmd_exit_0 node bin/differential_gate.mjs` + `evidence_recorded differential` + `json_in differential.verdict ∈{equivalent,divergences_enumerated,not_applicable}`

## 禁止
- SORT/collation 出力に order:multiset を当てて順序差を消すこと(class F は保存対象)
- equivalence 規則を後から緩めて divergence を黙らせること(reconcile で署名裁定せよ)
- 状態ファイル直接編集 / 禁止語
