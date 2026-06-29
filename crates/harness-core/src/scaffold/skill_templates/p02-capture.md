---
type: skill
title: "preservation / capture"
description: "このノードのゴール: 旧システムの実挙動を I/O 境界で golden 捕獲する。oracle = 旧の実挙動(bug-for-bug)。"
tags: [skill, workflow]
---

# skill: preservation / capture

このノードのゴール: 旧システムの実挙動を I/O 境界で **golden 捕獲**する。oracle = 旧の実挙動(bug-for-bug)。

★**cardinal sin 回避**: capture 時に normalize / sort / blanket-volatile-drop を**一切しない**(raw bytes 保存)。
これらは保存対象(class F collation/SORT 順)を capture 段で破壊し、腐った golden に全 differential が一致する
false-green を量産する。等価吸収は differential 側 equivalence.json の per-field opt-in に**後置**する。

## 順序

1. **capture_plan の partition を golden 化** ── 各代表 input を旧系へ流し raw 保存:
   ```
   node bin/capture_oracle.mjs record <旧系base> <path> --n 5 --id <partition-id>
   ```
   `golden/manifest.json` に {id, path, status, body(raw), provenance} 二層で追記される(★normalize 不使用)。

2. **非決定 field を分類**(★skeptic1/4 fix) ── `--n K`(K≥2)録画で K 回の per-field 分散を見る。
   N 録画間で**変動する field**は非決定(class E 未初期化 / G 採番 / F tie 順)と判定され
   `captured_nondeterministic=true` + `nondeterminism_ledger.json` に列挙される。
   その field は**別 cold セッションの独立評価者**が等価 policy(volatile/multiset/tolerance)を署名する
   (`nondeterminism_ledger.json` の該当 id に `evaluator:"independent"` + `policy`)。byte-identical 2回録画では非決定を見逃す。

3. **挙動中立 PII マスク** ── 本番データを録るなら桁長/型/符号/collation を保つ format-preserving mask のみ
   (VSAM キーの collation 順を壊さない)。mask_seed を provenance に記録。

4. **verify-provenance(ゲート)**:
   ```
   node bin/capture_oracle.mjs verify-provenance
   ```
   golden 無(旧系未到達=legit)→ N/A。到達したが非決定 field に署名 policy 無し → exit≠0(captured_nondeterministic 強制)。

5. evidence + 進める:
   ```
   harness report-evidence oracle_captured '{"verdict":"captured","entries":N,"nondeterministic":M}'
   harness advance
   ```
   verdict: captured / recaptured(再録画) / captured_nondeterministic(非決定あり・署名済) / not_applicable(旧未到達)。

## 完了条件(exit_gates)
- `cmd_exit_0 node bin/capture_oracle.mjs verify-provenance` + `evidence_recorded oracle_captured` + `json_in oracle_captured.verdict ∈{captured,recaptured,captured_nondeterministic,not_applicable}`

## 禁止
- capture 時に normalize / sort / blanket volatile drop すること(保存対象の破壊=false-green の温床)
- 非決定 field を env-freeze の closed set で握れると仮定すること(k≥N 録画で分類せよ)
- PII マスクで桁長/型/符号/collation を変えること(挙動を変える)
- 状態ファイル直接編集 / 禁止語
