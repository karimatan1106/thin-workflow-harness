# skill: security

このノードのゴール: 実装変更がセキュリティリスクを持たないか **3 層** で確認し、
`security_review` evidence を提出する。secret scan / dep audit を含む。

> 設計思想は Anthropic 公式 security-guidance プラグインの 3 層に対応する。
> harness は exit_gate で **強制**できるのが本家との差 ── 通らなければ次ノードへ進めない。

## 独立評価者が verdict を出す（同一 LLM 自己採点の禁止 ── generator/evaluator 分離）

**判定（approved/rejected）は、実装を生成した本スレッドが出してはならない。** 新鮮な
敵対的サブエージェント（`Agent` ツール）に出させ、本スレッドはその verdict を **relay
するだけ**。生成した本人の context は自己説得で埋まっており、自己採点は approved に
流れる（self-preferential bias）。test/verify は harness が gate を再実行/観測する決定論
センサで無問題だが、security の判定は決定論再実行できないため独立評価者で代替する。

**委譲する（評価者サブエージェント）:**

- input: `git diff main..HEAD`、層 2 の 8 クラス checklist、層 3 のデータフロー観点、変更ファイル一覧
- 指示（評価者の stance）: 「お前は**敵対的セキュリティレビュアだ。この変更は安全でないと
  仮定**し、安全だと証明されるまで approved を出すな。褒めるな。8 クラス各項を**実際に確認**
  （該当 path を読み injection/IDOR/SSRF/secret を具体的に追う）。dep audit は**自分で実行**
  して出力を貼れ（読むだけ不可）。全クラスクリアのときだけ verdict=approved、一つでも
  残れば rejected ＋ 各理由」
- **公平性の本体は fresh context / 別セッション**（共有 context を断てば自己説得の連鎖が消え、
  同一モデルでも公平に判定できる）。**評価者は fork 禁止**（`subagent_type:"fork"` は本スレッドの
  context を継承し自己採点へ逆戻りする ── 非 fork の fresh agent を使う）。別モデルは**系統的な
  共有盲点への追加防御**で必須でない。最良は **review/security を別セッションで cold に回す**
  （`harness status --run <id>` で pickup、または `harness run` worker）── 分離がプロセス事実に
  なり evidence 捏造の余地も消える。
- return（構造化）: `{verdict, evaluator_model, observed:[実行した audit 等と結果],
  layers:{l1_static,l2_diff,l3_dataflow}, risk_items[], notes}`

**本スレッドに残す（relay と操舵のみ）:**

- 評価者の verdict を**そのまま** `report-evidence security_review` に載せる。
  **approved への書き換え禁止**（評価者が rejected なら本スレッドで approved にしない。
  より厳しい rejected へ倒すのは可）。
- evidence に `evaluator:"independent"` / `evaluator_model` / `observed[]` を必ず含める
  （出口 gate `json_has security_review evaluator eq independent` が必須化）。
- `harness back` / `advance` / `harness ask` の操舵。
- 注: secret scan（gitleaks）は exit_gate `cmd_exit_0` が advance 時に再実行するので、
  二重に harness が握る（評価者は自分で走らせて貼り、harness が再実行して確定）。

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

exit_gate `evidence_recorded { key = "security_review" }` ＋
`json_has security_review evaluator eq independent` が pass するように、
**評価者サブエージェントが返した verdict をそのまま** relay する:

```
harness report-evidence security_review '{"verdict":"approved","evaluator":"independent","evaluator_model":"<別モデル tier>","observed":["cargo audit -> 0 CVE"],"layers":{"l1_static":"clear","l2_diff":"clear","l3_dataflow":"clear"},"risk_items":[],"notes":"..."}'
```

評価者が risk を残したら（`verdict:"rejected"`）、**本スレッドで approved に書き換えず**
そのまま relay し `harness back "security risk: ..."`。

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

- **本スレッド（生成者）が自分で 8 クラスを採点して verdict を出すこと**（判定は独立した
  敵対的評価者サブエージェントが出す。本スレッドは relay のみ）
- **評価者の rejected を本スレッドで approved に格上げすること**（より厳しい側へ倒すのは可）
- 「change は小さいから skip」で checklist を埋めないこと（最低限の確認は必須）
- 8 クラスのうち未確認のものを確認済として evidence に書くこと（gate 偽装）
- risk_items を空にしたいだけで rejected を approved に書き換えること（gate 偽装）
- secret を redact せず log / artifact に残すこと
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名（`evidence_recorded` 等）を
  渡すこと ── 渡すのは evidence の **key 名**（`security_review`）
