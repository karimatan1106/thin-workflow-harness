# skill: security

このノードのゴール: 実装変更がセキュリティリスクを持たないか **3 層** で確認し、
`security_review` evidence を提出する。secret scan / dep audit を含む。

> 設計思想は Anthropic 公式 security-guidance プラグインの 3 層に対応する。
> harness は exit_gate で **強制**できるのが本家との差 ── 通らなければ次ノードへ進めない。

## サブエージェント隔離（diff レビュー・監査）

層 2（差分レビュー）・層 3（データフロー深掘り）・dep audit の **読み込み/解析**は
`Agent` ツールに委譲し、構造化 findings レポートだけを持ち帰る（diff 本文で本スレッドの
context を汚さない）。

- 委譲する: `git diff` 抽出と 8 クラス checklist の読込採点・cross-file データフロー追跡・
  `cargo audit`/`pnpm audit` の実行と解析。サブエージェントには「{8 クラス毎の verdict,
  risk_items[], dep CVE} を構造化して返せ。diff 本文は貼るな」と指示する。
- 本スレッドに残す: 判定（approved/rejected）・`report-evidence security_review`・
  `harness back` / `advance`・`harness ask` の判断。
- 注: secret scan（gitleaks）は exit_gate `cmd_exit_0` が advance 時に再実行するので、
  gate 判定は harness が握る（サブエージェントは解析のみ）。

## 前提

- test phase が緑（cmd_exit_0 で test スイート pass 済）

## 3 層レビュー

### 層 1 ── 静的パターン（コスト0・既に自動実行済）

harness は implement 中の `edit_file` / `create_file` のたびに content をローカル
substring scan し、危険パターンに warning を返している（LLM 呼び出しなし）。検出クラス:
injection / XSS / 危険デシリアライズ / 弱い暗号 / TLS 無効化 / hardcoded secret。

- このノードでは **層1 warning が出ていた箇所が解消済みか**を最初に確認する。
- 残っているなら直すか、安全な理由をコード内コメントに残す。

### 層 2 ── 差分レビュー（このノードの主作業）

diff を抽出し、下の 8 クラスで読む:

```
git diff main..HEAD --stat
git diff main..HEAD -- 'src/' | head -200
```

セキュリティ checklist（8 クラス）:

- [ ] **injection** ── SQL / command / path-traversal。文字列連結でクエリ・shell を組んでいない
- [ ] **XSS** ── innerHTML / dangerouslySetInnerHTML / document.write を untrusted 入力で使っていない
- [ ] **認可バイパス** ── 認証/認可 path の変更が intentional + reviewed
- [ ] **IDOR** ── リソース ID を受けて操作する箇所で所有者/権限チェックがある
- [ ] **SSRF** ── user 制御 URL への fetch/request に allowlist がある
- [ ] **危険デシリアライズ** ── pickle / yaml.load / torch.load 等を untrusted データに使っていない
- [ ] **弱い暗号** ── MD5/SHA1 を署名・パスワードに使わない、AES-ECB を使わない、TLS 検証を無効化しない
- [ ] **secret 露出** ── credentials / API key の hard-code なし、log に token/PII/内部 URL が出ない

加えて:
- [ ] dependency 追加が intentional、license / supply chain 確認済
- [ ] file I/O の path validation あり（`../` guard）

### 層 3 ── データフロー深掘り（multi-file 脆弱性）

パターンマッチで漏れる cross-file の問題を、関連ファイルを跨いで追う:

- IDOR / 認可バイパス: 入力が handler → service → DB へ流れる経路で権限チェックが**どこか**にあるか
- SSRF: user 入力が最終的に外向き request に届くまでに sanitize されるか
- 必要なら `read_file` / `run_command("grep ...")` で呼び出し元・呼び出し先を辿る

## secret scan を走らせる（exit_gate で配線済）

```
gitleaks detect --no-git --redact
```

exit 0 で `cmd_exit_0` gate が pass。検出があれば redact / 削除して再実行。

## dependency audit（任意・推奨）

```
cargo audit                       # Rust
pnpm audit --prod                 # Node.js
pip-audit                         # Python
```

既知 CVE があれば判断（critical/high は基本 reject）。

## 判定 evidence を提出

exit_gate `evidence_recorded { key = "security_review" }` が pass するように:

```
harness report-evidence security_review '{"verdict":"approved","layers":{"l1_static":"clear","l2_diff":"clear","l3_dataflow":"clear"},"risk_items":[],"notes":"..."}'
```

risk が残るなら `verdict: "rejected"` で `harness back "security risk: ..."`。

## 進める

```
harness advance
```

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "security"` を参照）:

- `cmd_exit_0 { cmd = "<security-scan>" }` ── secret scan が exit 0
  （未検出なら `false # configure security-scan ...` ── gitleaks インストール推奨）
- `evidence_recorded { key = "security_review" }` ── 上の `report-evidence` で pass

## 詰まったとき

- gitleaks が誤検出 → false positive を redact ＋ notes に rationale を書く
- dep の CVE が修正不能 → `harness ask` で許容判断、または `harness back "dep 入替"`
- これ以上進めない → `harness stuck "<理由>"`

## 禁止

- 「change は小さいから skip」で checklist を埋めないこと（最低限の確認は必須）
- 8 クラスのうち未確認のものを確認済として evidence に書くこと（gate 偽装）
- risk_items を空にしたいだけで rejected を approved に書き換えること（gate 偽装）
- secret を redact せず log / artifact に残すこと
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名（`evidence_recorded` 等）を
  渡すこと ── 渡すのは evidence の **key 名**（`security_review`）
