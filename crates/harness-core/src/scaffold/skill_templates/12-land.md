---
type: skill
title: "land"
description: "このノードのゴール: run の work branch を既定ブランチ（main / master）に畳み（merge）、"
tags: [skill, workflow]
---

# skill: land

このノードのゴール: run の work branch を既定ブランチ（main / master）に畳み（merge）、
リモートへ push し、work branch を local / remote とも削除する ── **ここまでを 1 セットの
「着地（land）」終了処理**として行う。default workflow には現れない（**opt-in**）。
plan の `can_append = true` で workflow.toml の終端に追記して有効化する（下「有効化」）。

## 前提

- review / docdesign など先行の品質ゲートが全て緑（着地は最後の最後）
- working tree がクリーン（未コミットの変更が無い ── あるなら docdesign / implement で commit 済）

## 安全原則（壊さないための鉄則）

- merge は **`--no-ff`**（着地を 1 つの merge commit として履歴に残す）
- branch 削除は **`git branch -d`（マージ済みでなければ削除を拒否）**。**`-D` 厳禁**（未マージ作業が飛ぶ）
- conflict / 未マージ / dirty tree なら **握り潰さず中断してエスカレーション**（`harness stuck`）
- **work branch == 既定ブランチ（main 直で走った）なら no-op = `verdict: "not_applicable"`**
  （畳む対象が無い ── 詰まらせず正しく抜ける）

## 順序

1. **保留 gate を確認**
   ```
   harness status
   ```

2. **状況把握**（read-only）
   ```
   work=$(git symbolic-ref --short HEAD)                                  # 現 work branch
   base=$(git remote show origin | sed -n 's/.*HEAD branch: //p')         # 既定ブランチ（無ければ main/master を検出）
   git status --porcelain                                                  # 空であること（dirty なら中断）
   ```
   - `work == base`（main 直で走った）→ **step 5 へ（not_applicable）**
   - dirty tree → docdesign / implement で commit してから戻る

3. **着地（1 セット）** ── 1 つでも失敗したら以降を止めてエスカレーション
   ```
   git switch <base>
   git merge --no-ff <work>            # conflict → harness stuck "land conflict: ..."（自動解決しない）
   git push origin <base>              # 既定ブランチをリモートへ
   git branch -d <work>               # マージ済みのみ削除（-d が安全弁）
   git push origin --delete <work>     # リモート work branch も削除（存在すれば）
   ```

4. **着地後の最終確認**（任意・推奨）
   ```
   git log --oneline -1                # merge commit を確認
   ```

5. **land evidence を提出** ── exit_gate `evidence_recorded { key = "land" }` ＋
   `json_in { evidence_key = "land", json_path = "verdict", one_of = "landed,not_applicable" }`:
   ```
   # 着地した場合
   harness report-evidence land '{"work_branch":"<work>","base":"<base>","merged":true,"pushed":true,"branch_deleted":true,"verdict":"landed"}'

   # main 直で走った場合（畳む対象なし）
   harness report-evidence land '{"verdict":"not_applicable","reason":"already on base branch"}'
   ```

6. **終端** ── `next = []`。完了は harness が `land` gate met を検知して出る
   （`harness status` で「全 gate met」を確認）。

## 完了条件（exit_gates）── `workflow.toml` の `[[node]] id = "land"`

- `evidence_recorded { key = "land" }`
- `json_in { evidence_key = "land", json_path = "verdict", one_of = "landed,not_applicable" }`
- （任意・teeth）`cmd_exit_0 { cmd = "git diff --quiet && git diff --cached --quiet" }`
  ── 着地後に working tree がクリーン（畳み残しが無い）

## 有効化（opt-in の配線手順）

plan ノードで `can_append = true` を使い、終端ノード（review か docdesign）の `next` を
`["land"]` に変え、末尾に以下を append する（`workflow_append_only` gate に違反しないこと）:

```
[[node]]
id = "land"
skill = "12-land.md"
# 機械的な git 操作中心なので Sonnet 据え置きで可。
exit_gates = [
  { gate = "evidence_recorded", args = { key = "land" } },
  { gate = "json_in", args = { evidence_key = "land", json_path = "verdict", one_of = "landed,not_applicable" } },
  { gate = "cmd_exit_0", args = { cmd = "git diff --quiet && git diff --cached --quiet" } },
]
next = []
on_reject = { after = 2, goto = "__human__" }
```

## 詰まったとき

- merge conflict → `harness stuck "land conflict: <要点>"`（自動解決せず人間へ）
- dirty tree → 変更を docdesign / implement で commit してから戻る
- push 権限 / 認証エラー → `harness ask "<質問>" --option ... --option ...`
  （**ローカルの merge + branch 削除は完了している旨を notes に残す**）

## 禁止

- **`-D` での強制削除**（未マージ作業が飛ぶ ── 削除は必ず `-d`）
- conflict をテスト / コード書き換えで握り潰すこと
- `work == base` での無理な merge / 削除（`not_applicable` で正しく抜ける）
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名を渡すこと
  ── 渡すのは evidence の **key 名**（`land`）
