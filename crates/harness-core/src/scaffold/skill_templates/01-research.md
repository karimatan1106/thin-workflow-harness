# skill: research

このノードのゴール: 改修対象を理解し、検証可能な `spec.toml` を作って人間の承認を取る。
コードは編集しない。**壁打ち / scope のループ**であり、唯一「無制限の人間対話が OK」な場所
── ここで over-ask しろ、間違った実装より安い。決まったら即 `spec.toml` に書いて context
から出す（spec は結晶化した壁打ち）。

## 順序

1. **意図の言い直し**（最初、コードを読む前）── 生の intent（`harness status` に出る）を
   自分の言葉で言い直し、人間に確認する:
   ```
   harness ask "この理解で合ってる?（要約: ...）" --option "合ってる" --option "ずれてる"
   ```

2. **scope（blast radius の発見）** ── CKG (Code Knowledge Graph) tool で候補集合を作る:
   ```
   harness outline <file>                          # アウトライン（本体ではない）
   harness find-symbol <name>                      # シンボル位置
   harness closure <sym> --depth N                 # 呼び出し下流
   harness impacted-by <sym>                       # 署名変更時の上流影響
   harness show-symbol <sym>                       # 本体（少数の触りそうなシンボルだけ）
   ```
   grep は使わない（位置でなくテキストが返り context が膨らむ）。候補集合を
   `requirement.files` のドラフトにして人間に確認:
   ```
   harness ask "blast radius はこれで漏れは?" --option "OK" --option "漏れあり"
   ```

3. **不変条件の特定**（何を壊しちゃダメか）── 各々 `[[invariant]]`（INV-N）として
   `spec.toml` に書き、それぞれに `test` を紐づける。不確かなら `harness ask` で訊く。

4. **受入基準** ── 各々 `[[acceptance]]`（AC-N、`requirement` に紐づく F-ID＋`test` 1 つ）。
   「all AC テスト green」≡「意図した変更」になる程度に具体的に書く。曖昧な AC は smell
   （テスト化できない AC は AC でない）。

5. **残った曖昧さ** ── `??` で `spec.toml` 本文に書き、`harness ask` で潰す。
   **決定を訊け、情報を訊くな** ── コードで分かることは自分で見つけよ、人間に訊くのは
   判断だけ。`open_questions_zero` gate は `??` が無くなるまで fail。

6. **最終承認** ── spec 全体を提示して人間 sign-off:
   ```
   harness ask "この spec、欲しい変更か?（要約: F-NNN/AC/不変条件/blast radius）" \
              --option "承認" --option "修正が要る"
   ```
   承認が返ったら evidence を提出:
   ```
   harness report-evidence human_approval '{"verdict":"approved","rationale":"..."}'
   ```

7. 調査メモを artifact 登録（semantic クエリで得た blast radius・依存・判断根拠の蒸留）:
   ```
   harness record-artifact research_notes <path>
   ```

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "research"` を参照）:

- `open_questions_zero` ── 未解決点ゼロ、`??` なし
- `no_pending_required_questions` ── 保留中の `harness ask` がない
- `json_has human_approval verdict == "approved"` ── 上の `report-evidence` で pass
- （あれば）`blast_radius_declared` ── 各 F-NNN に `files` ≥1

満たしたら `harness request-transition plan`。却下 (`advance_rejected`) されたら
`failed_gates` の理由を読んで直し、もう一度 `request-transition`。

## 詰まったとき

これ以上進めない（gate が満たせない・前提が崩れている・情報が足りない）と判断したら、
`harness request-transition` を空打ちで繰り返さず正直にエスカレせよ:
```
harness stuck "<理由>"
```
harness が `node_aborted` を書いて人間に回す。

## 禁止

- このフェーズでコードを編集すること（research のみ）
- 状態ファイル（イベントログ・他人が書く `spec.toml` 箇所）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report_evidence` の `gate` 引数に gate プリミティブ種別名（`evidence_recorded` 等）を
  渡すこと ── 渡すのは evidence の **key 名**（`human_approval` 等）
