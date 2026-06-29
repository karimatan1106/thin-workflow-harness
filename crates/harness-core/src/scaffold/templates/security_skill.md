---
type: skill
title: "security (standalone)"
description: "このノードのゴール: 現在の変更（または指定範囲）に security リスクが無いか 3 層 で"
tags: [skill, workflow]
---

# skill: security (standalone)

このノードのゴール: 現在の変更（または指定範囲）に security リスクが無いか **3 層** で
確認し、`security_review` evidence を提出して done にする。

> Anthropic 公式 security-guidance プラグインの 3 層を harness に移植したもの。
> harness は exit_gate で **強制**できる ── evidence 無しでは done にできない。

## 3 層レビュー

### 層 1 ── 静的パターン（コスト0・engine が自動実行済）

harness は `edit_file` / `create_file` のたびに content をローカル substring scan し、
危険パターンに warning を返している（LLM 呼び出しなし）。検出クラス:
injection / XSS / 危険デシリアライズ / 弱い暗号 / TLS 無効化 / hardcoded secret。
→ warning が出ていた箇所が解消済みか最初に確認する。

### 層 2 ── 差分レビュー（主作業・8 クラス）

```
git diff --stat
git diff -- 'src/' | head -200
```

- [ ] **injection** ── SQL / command / path-traversal。文字列連結でクエリ・shell を組まない
- [ ] **XSS** ── innerHTML / dangerouslySetInnerHTML / document.write を untrusted 入力で使わない
- [ ] **認可バイパス** ── 認証/認可 path の変更が intentional + reviewed
- [ ] **IDOR** ── リソース ID 操作で所有者/権限チェックがある
- [ ] **SSRF** ── user 制御 URL への fetch/request に allowlist がある
- [ ] **危険デシリアライズ** ── pickle / yaml.load / torch.load 等を untrusted データに使わない
- [ ] **弱い暗号** ── MD5/SHA1 を署名・パスワードに使わない、AES-ECB 不使用、TLS 検証無効化なし
- [ ] **secret 露出** ── credentials / API key の hard-code なし、log に token/PII/内部 URL なし

### 層 3 ── データフロー深掘り（multi-file 脆弱性）

パターンマッチで漏れる cross-file の問題を関連ファイルを跨いで追う。
IDOR/認可は入力→handler→service→DB の経路で権限チェックがどこかにあるか、
SSRF は user 入力が外向き request に届くまでに sanitize されるか。必要なら
`read_file` / `run_command("grep ...")` で呼び出し元・先を辿る。

## secret scan（推奨）

```
gitleaks detect --no-git --redact
```

検出があれば redact / 削除する。

## 判定 evidence を提出して done

```
harness report-evidence security_review '{"verdict":"approved","layers":{"l1_static":"clear","l2_diff":"clear","l3_dataflow":"clear"},"risk_items":[],"notes":"..."}'
harness request-transition
```

risk が残るなら `verdict:"rejected"` にし、`risk_items` に具体箇所を列挙して修正してから再提出する。

## 禁止

- 8 クラスのうち未確認のものを確認済として evidence に書くこと（gate 偽装）
- risk_items を空にしたいだけで rejected を approved に書き換えること
- secret を redact せず log / artifact に残すこと
- 状態ファイル（イベントログ）の直接編集
- `report-evidence` の `gate` 引数に gate 種別名を渡すこと ── 渡すのは key 名 `security_review`
