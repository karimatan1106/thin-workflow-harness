# skill: plan

このノードのゴール: `spec.toml` に従い、**徹底的な**実装計画を `plan` artifact（≤200 行）に
まとめ、人間の plan 承認を取る。コードは編集しない。これは人間チェックポイント 2 つ目
（spec 承認に続く）── heavyweight に書け。

**plan の grilling（grill-with-docs 方式）── plan モード(ホスト機能)は使わない。** 代わりに
計画をドメイン言語と実装に対して執拗に尋問する: 計画の各面を **1 問ずつ `harness ask`**(推奨答えを
option の先頭に含める)で詰め、回答を待ってから次へ。コードで分かることは訊かず `harness-lspd query`
/ Read で確認。曖昧/過負荷の語は即指摘し **正典名を提案** して `CONTEXT.md`(用語集、形式は
`docs/CONTEXT-FORMAT.md`)にその場で追記。**具体シナリオでエッジを突き** 概念の境界を強制的に
明らかにする。述べた挙動は **実装と照合** し矛盾を表に出す。コードは編集しない。

**ADR は 3 条件が全て成立する時だけ起票** ── (1) 後戻り困難 (後で変える代償が大きい) /
(2) 文脈無しでは意外 (将来の読者に説明が要る) / (3) 本物のトレードオフ (実在する代替があった)。
1 つでも欠ければ ADR を作らない。

## 順序

1. **spec スライスを読む** ── 直前 phase で fix された spec を確認:
   ```
   harness status                     # 現ノードの保留 gate・担当 F-NNN
   harness spec <F-NNN>               # 該当 F-NNN の requirement / AC / invariant
   ```
   必要なら構造を再確認（CKG=harness-lspd、未導入なら `harness setup-ckg`。本体は読まない、形だけ）:
   ```
   harness-lspd query outline <file>
   harness-lspd query closure <sym> --depth N
   ```

2. **`plan` artifact（≤200 行）を書く** ── 最低限以下を含めること:
   - 変更ファイル一覧と各々の責務 / 新規ファイル一覧と各々の責務
   - 変更の順序（依存順）
   - 各 `AC-N` ↔ それを担保する test コマンドの対応
     （漏れがあれば spec が不完全 ── research へ `harness back "..."`）
   - リスク（壊しうる不変条件、隠れた依存、blast radius の漏れの可能性）
   - 代替案の検討（なぜこのアプローチか、却下した案と理由）
   - rollback 戦略（壊れたときどう戻すか）

3. **分解**（計画が大きければ）── `F-007` を blast radius が互いに素な `F-007.1`（ファイル
   A,B）/ `F-007.2`（ファイル C,D）に分解 → `spec.toml` に追記し、`workflow.toml` に並列
   ノード（`fork` / `join`）を追加する。**各 fork は vertical slice にする**(to-issues 由来): 各 `F-NNN.x` は
   (a) 自分の AC 部分集合を持ち **単体で全 green になる縦切り**(=兄弟 slice 無しでは AC が通らない切り方=
   横切り[片レイヤだけ]の smell として却下)、(b) blast radius が demoable な end-to-end 経路を成すこと。
   横切りは `traceability_closed`(各 slice に exit0 test ≥1)と `blast_radius_disjoint` が機械的に弾く。
   plan ノードは `can_append = true` なので
   `workflow.toml` に新規ノード追加可 ── ただし `workflow_append_only` の範囲内のみ
   （既存ノード・既存 gate を弱められない／変えられるのは未到達ノードへの配線追加だけ／
   新規ノードは `[meta].mandatory_gates` を満たすこと）。判断に迷うときは `harness ask`。

4. **plan artifact 登録**:
   ```
   harness record-artifact plan <path>
   ```

5. **plan 承認を取る**:
   ```
   harness ask "この plan で進める?（要約: 変更ファイル/順序/AC↔test/リスク）" \
              --option "承認" --option "修正が要る"
   ```
   承認が返ったら evidence を提出:
   ```
   harness report-evidence plan_approval '{"verdict":"approved","notes":"..."}'
   ```

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "plan"` を参照）:

- `artifact_registered { name_or_prefix = "plan" }` ── 上の `record-artifact` で pass
- `json_has plan_approval verdict == "approved"` ── 上の `report-evidence` で pass
- `workflow_append_only` ── workflow.toml の改変は追加のみ・既存緩和なし
- （あれば）`max_lines plan.md 200` ── ≤200 行に収める
- （あれば）`traceability_closed` ── 各 F-NNN に artifact ≥1 と exit 0 test ≥1、orphan
  なし

満たしたら `harness advance`（無ければ `implement`）。

## append-only の注意

plan artifact は次 phase 以降で **読み取り専用**。一旦 `record-artifact` したら上書き
しない。修正が必要なら新しい artifact (`plan_v2`) を作るか、`harness back "..."` で前
phase に戻る。**`workflow.toml` の既存ノードを変えない／既存 gate を弱めない**
（`workflow_append_only` がこれを強制する）。

## 詰まったとき

`harness advance` を空打ちで繰り返さない。spec の前提が崩れていたら
`harness back "..."` で research へ。これ以上進めないなら `harness stuck "<理由>"`。

## 禁止

- このフェーズでコードを編集すること（具体的 code 編集は implement で）
- 過大な scope（1 plan で 50+ file 変更等）── 警戒、必要なら分割
- 状態ファイル（イベントログ・他人が書く `spec.toml` 箇所）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
