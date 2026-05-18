# Skill: done

最小 example ── 1 phase で `report_evidence` を 1 回呼んで完了。

## ゴール

`completed` evidence を harness に記録する。

## 手順

1. `report_evidence` を呼ぶ:
   - `gate`: `"completed"` (evidence key 名。 gate 種別名 `evidence_recorded` ではない)
   - `json`: `{"status": "ok"}`
2. `request_transition` を呼ぶ ── exit_gates の `evidence_recorded { key = "completed" }` が pass し、 next=[] なので workflow 終了。

## 重要

- `report_evidence(gate="evidence_recorded", ...)` は **誤り** ── `gate` 引数には evidence の *key 名* を渡す（ docs/skill-templates.md §「全 skill 共通の前提」 §最後）。
- 詰まったら `stuck "<理由>"` でエスカレせよ。 `request_transition` を空打ちで gate を満たそうとするな。
