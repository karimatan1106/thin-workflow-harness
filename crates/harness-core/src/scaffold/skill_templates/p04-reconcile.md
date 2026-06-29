---
type: skill
title: "preservation / reconcile"
description: "このノードのゴール: state/divergences.json の各 divergence を 3値で署名裁定し state/reconcile_ledger.json へ。"
tags: [skill, workflow]
---

# skill: preservation / reconcile

このノードのゴール: `state/divergences.json` の各 divergence を **3値で署名裁定**し `state/reconcile_ledger.json` へ。
未署名が1件でも残れば止まる(silent-divergence 禁止=signed-empty-set)。

★**裁定は生成本人でなく別 cold セッションの独立評価者(ADR-059)が署名する**。本スレッドは relay のみ・
approved への格上げ禁止。「旧と同じはずが違う」divergence は移植バグの最有力候補=自己採点は危険。

## 3値裁定の要件

| status | 意味 | 必須フィールド |
|---|---|---|
| `preserve_quirk` | bug-for-bug 保存(旧の quirk を再現せよ) | `test`(旧挙動を pin する実テスト) |
| `accepted_env_diff` | 容認した環境差(EBCDIC→ASCII 等) | `positive_fixture`(規則前 old≠new・後 ==) + `discriminating_witness`(規則が保存すべき既知隣接バグ) + `scope` |
| `intentional_fix` | 意図的な挙動変更 | `adr`(ADR-NNN) + `downstream`(下流影響) |

★`accepted_env_diff` は「規則 R 下で old==new」(循環・aggressive な R なら何でも成立)では**不可**。
positive fixture と discriminating witness の**両指名**が要る(skeptic2 fix)。独立評価者へは survivor でなく
「吸収集合+各規則の discriminating witness」を渡し『この規則が隠す本物の divergence を探せ』と命じる。

## 順序

1. **独立評価者を起動**(別 cold セッション / 非 fork の fresh agent)。divergences.json と
   absorbed_divergences.json + 各規則の witness を渡す。「旧挙動を正とし、各 divergence を3値で裁定。
   本物の移植バグを accepted_env_diff/intentional_fix と誤署名するな」。
2. 各 divergence id に署名 record を `state/reconcile_ledger.json` へ追記(行番号不使用・content-hash で失効再裁定)。
3. ゲート確認 + evidence:
   ```
   node bin/reconcile_gate.mjs        # 未署名/不備があれば exit1
   harness report-evidence reconcile '{"verdict":"all_signed","evaluator":"independent","signed":N}'
   harness advance
   ```
   verdict: all_signed / no_divergence / not_applicable。`evaluator` は `independent` 必須(json_has)。

## 完了条件(exit_gates)
- `cmd_exit_0 node bin/reconcile_gate.mjs` + `evidence_recorded reconcile` + `json_in reconcile.verdict ∈{all_signed,no_divergence,not_applicable}` + `json_has reconcile.evaluator == independent`

## 禁止
- 生成本人が自分で裁定して署名すること(別 cold セッションの独立評価者が署名)
- **本物の移植バグ(旧と挙動が割れた)を accepted_env_diff / intentional_fix と誤署名すること**
- accepted_env_diff を「R 下 old==new」だけで通すこと(positive fixture + discriminating witness 必須)
- evaluator フィールドを independent と詐称すること(別セッション運用で構造的に減らす)
- 状態ファイル直接編集 / 禁止語
