# Skill: report

調査結果を確認して完了 evidence を登録する。

## 手順

1. **直前 phase の artifact 確認**:
   ```
   read_file("exploration-summary.md")
   ```
   `explore` ノードで登録された artifact の中身を読む。

2. **完了 evidence 登録**:
   ```
   report_evidence(gate="completed", json={"status":"done", "summary":"<1 行要約>"})
   ```
   - `gate` 引数は evidence の *key 名* (`completed`)。 gate 種別名 (`evidence_recorded`) ではない。
   - exit_gates の `evidence_recorded { key = "completed" }` が pass する。

3. **遷移**:
   - `request_transition` を呼ぶ ── next=[] なので workflow 終了。

## 重要

- このノードでは `edit_file` を使わない (報告だけ、 コード変更なし)。
- exploration-summary に禁止語 (TODO/TBD/未定/サンプル 等) が残っていたら、 `back "禁止語が残存"` で `explore` に戻す。
