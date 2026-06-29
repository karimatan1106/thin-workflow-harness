---
type: reference
title: "docs/schemas.md — スキーマ"
description: "> これは暫定確定スキーマである。フィールド名・型・必須/任意を以下に定める。実装時に微調整されうるが、大枠はこれ。DESIGN.md と合わせて読むこと。"
tags: [harness, docs]
---

# docs/schemas.md — スキーマ

> **これは暫定確定スキーマである。フィールド名・型・必須/任意を以下に定める。実装時に微調整されうるが、大枠はこれ。`DESIGN.md` と合わせて読むこと。**

## 1. `spec.toml` スキーマ

設計書の*検証可能な*部分を構造化したもの（`DESIGN.md` §6）。トレースマップは `[[requirement]]` の `files` / `tests` ＋ 実行時に `record-artifact <name> <path>` で登録される artifact で表現する（F-NNN → blast radius のファイル → 検証 test コマンド、＋実装した artifact）。AC は `requirement`（F-ID）に紐づき、各々が自分を検証する `test` を 1 つ持つ。`invariant` も AC と同様にテスト化する。

```toml
[meta]
intent = "ログイン処理のリファクタリング"   # 人間が出した変更依頼の一行
status = "draft"                            # "draft" | "frozen" — frozen は壁打ち完了かつ人間承認済

# --- 要件（F-NNN）。files = blast radius（影響ファイル一覧）、tests = 検証コマンド ---
[[requirement]]
id = "F-001"                                # 要件 ID（F-\d+）
text = "認証ロジックを auth モジュールに集約する"   # 1行〜数行
files = ["src/auth/mod.rs", "src/auth/hash.rs"]   # この要件の blast radius（影響/新規ファイルのパス）
tests = ["cargo test auth::"]               # この要件を検証する test コマンドのリスト（cmd_exit_0 で実行される）
rationale = "認証分散による重複と漏れを解消"   # 任意

[[requirement]]
id = "F-002"
text = "セッショントークンの検証を独立した関数にする"
files = ["src/session/verify.rs"]
tests = ["cargo test session::verify"]

# --- 受入基準（AC-N）。各 AC は requirement に紐づき、自分を検証する test を 1 つ持つ ---
[[acceptance]]
id = "AC-1"                                 # AC-\d+
requirement = "F-001"                       # どの要件の受入基準か（F-ID 参照）
text = "auth モジュール外から直接パスワードハッシュを参照していない"
test = "cargo test ac1::no_external_hash_ref"   # この AC を検証する test コマンド

[[acceptance]]
id = "AC-2"
requirement = "F-002"
text = "不正トークンで verify が false を返す"
test = "cargo test ac2::reject_invalid_token"

# --- 改修で維持すべき不変条件（AC と同様にテスト化する）---
[[invariant]]
id = "INV-1"                                # INV-\d+
text = "既存ユーザのセッションは改修後も有効のまま"
test = "cargo test inv1::existing_session_still_valid"

# --- 壁打ちの未解決点。配列が空 かつ 本文中に ?? が無い とき open_questions_zero gate が pass ---
[[open_question]]
id = "Q-1"
text = "リフレッシュトークンのローテーション方針は？"
options = ["毎回ローテート", "失効時のみローテート"]   # 任意（harness ask の選択肢に対応）
# answer = "毎回ローテート"                  # 任意。人間が harness answer で回答したら harness が埋める。未回答なら欠落

# --- 人間承認。report-evidence human_approval / harness answer(spec_approval) で埋まる ---
[approval]
verdict = ""                                # "approved" など
by = ""                                     # 承認者
notes = ""                                  # 補足
```

### 1.1 `spec.toml` フィールド表

| フィールド | 型 | 必須/任意 | 既定 | 意味 |
|---|---|---|---|---|
| `[meta].intent` | str | 必須 | — | 人間が出した変更依頼の一行 |
| `[meta].status` | `"draft"`\|`"frozen"` | 必須 | `"draft"` | `frozen` は壁打ち完了かつ人間承認済 |
| `[[requirement]].id` | str（`F-\d+`） | 必須 | — | 要件 ID |
| `[[requirement]].text` | str | 必須 | — | 要件本文（1 行〜数行） |
| `[[requirement]].files` | list[str] | 必須（空可） | — | blast radius（影響/新規ファイルのパス一覧） |
| `[[requirement]].tests` | list[str] | 必須（空可） | — | この要件を検証する test コマンド一覧（`cmd_exit_0` で実行） |
| `[[requirement]].rationale` | str | 任意 | — | 根拠 |
| `[[acceptance]].id` | str（`AC-\d+`） | 必須 | — | 受入基準 ID |
| `[[acceptance]].requirement` | str | 必須 | — | 紐づく F-ID |
| `[[acceptance]].text` | str | 必須 | — | 受入基準本文 |
| `[[acceptance]].test` | str | 必須 | — | この AC を検証する test コマンド |
| `[[invariant]].id` | str（`INV-\d+`） | 必須 | — | 不変条件 ID |
| `[[invariant]].text` | str | 必須 | — | 不変条件本文 |
| `[[invariant]].test` | str | 必須 | — | この不変条件を検証する test コマンド |
| `[[open_question]].id` | str | 必須 | — | 質問 ID |
| `[[open_question]].text` | str | 必須 | — | 質問本文 |
| `[[open_question]].options` | list[str] | 任意 | — | 選択肢（`harness ask` の `--option` に対応） |
| `[[open_question]].answer` | str | 任意 | （欠落） | 人間の回答。未回答なら欠落 |
| `[approval].verdict` | str | 任意 | `""` | `"approved"` 等。`report-evidence human_approval` / `harness answer`(spec_approval) で埋まる |
| `[approval].by` | str | 任意 | `""` | 承認者 |
| `[approval].notes` | str | 任意 | `""` | 補足 |

- **`open_questions_zero` gate**: `[[open_question]]` 配列が空 **かつ** どの `text` フィールド（`spec.toml` 本文中のどこ）にも `??` が無い とき pass。
- **トレースマップ**: requirement の `files` / `tests` ＋ 実行時に `record-artifact <name> <path>` で登録される artifact が、F-NNN ↔ ファイル ↔ test ↔ artifact の対応を成す。**`traceability_closed` gate** は ①各 F-NNN に「実在する artifact が ≥1」かつ「exit 0 する test コマンドが ≥1」②登録された artifact がどれかの F-NNN の `files` に含まれる（orphan なし）── の両方を検証する（`DESIGN.md` §6）。

## 2. `workflow.toml` スキーマ

プロセス（ノード/エッジ/出口 gate）の定義（`DESIGN.md` §5）。実行中に append できる（`workflow_append_only` gate で「追加のみ」を強制）。

### 2.1 `workflow.toml` フィールド表

| フィールド | 型 | 必須/任意 | 既定 | 意味 |
|---|---|---|---|---|
| `[meta].name` | str | 必須 | — | ワークフロー名 |
| `[meta].entry` | str | 必須 | — | 開始ノード id |
| `[meta].mandatory_gates` | list[`{gate=str, args=table}`] | 任意 | `[]` | 全ノード（または終端ノード）が含むべき gate spec のリスト。実行中に append された新規ノードはこれらを満たさなければならない（`workflow_append_only` が検証）。有力候補に `{ gate="cmd_exit_0", args={ cmd="cargo check --workspace" } }`（**workspace 全体** ── per-crate でなく workspace 全体にして domain をまたぐ署名 break を導入ノードで安く捕まえる）＋ `{ gate="cmd_exit_0", args={ cmd="gitleaks detect --no-git --redact" } }`（ソースに書いたシークレットを捕まえる）。`DESIGN.md` §5.1・§16.1・`docs/operations.md` §1/§2/§6 |
| `[meta].default_model` | str | 任意 | — | ノードに `model` が無いとき適用する worker のモデル（`DESIGN.md` §16.1） |
| `[meta].default_budget` | table | 任意 | — | ノードに `budget` が無いとき適用する予算（`{max_tool_calls, max_tokens, max_wall_seconds}`、`DESIGN.md` §16.1） |
| `[meta].run_cost_budget` | 数値 | 任意 | — | run 全体のコスト上限。超過で人間にエスカレ（`DESIGN.md` §16.1） |
| `[meta].secrets_glob` | list[str] | 任意 | `[]` | worker の context に入れないファイルの glob（`DESIGN.md` §16.2） |
| `[meta].host` | `"claude-code"`\|`"runtime"` | 任意 | `"runtime"` | ホストが何を組込みで提供するか。`"claude-code"` のとき各 skill はホストの plan モード／`/security-review`／`/review`／AskUserQuestion を優先し、無ければ skill 内の手順に従う。`"runtime"` は harness ランタイム自身がホスト（plan ノード skill・`skills/security-review.md`・`skills/code-review.md`・質問キュー・生 API sub-worker・tool-call インターセプタ）。`DESIGN.md` §10・`docs/host-capabilities.md` |
| `[[node]].id` | str | 必須 | — | ノード id |
| `[[node]].type` | `"task"`\|`"fork"`\|`"join"` | 任意 | `"task"` | ノード種別 |
| `[[node]].skill` | str | task は必須 / fork・join は任意 | — | `skills/` 配下のファイル名 |
| `[[node]].can_append` | bool | 任意 | `false` | このノードが実行中に `workflow.toml` を append（新規ノード追加・未到達ノードへの配線追加）してよいか。通常は plan ノードだけに `true` を付ける。append できる範囲は `workflow_append_only` gate が縛る（`DESIGN.md` §5.1） |
| `[[node]].serves` | list[str] | 任意 | `[]` | このノードが実装する F-ID 一覧（spec スライス計算と traceability に使う） |
| `[[node]].exit_gates` | list[`{gate=str, args=table}`] | 任意 | `[]` | このノードの出口 gate |
| `[[node]].next` | list[str] | 任意 | `[]` | 後続ノード id（task の分岐候補。空＝最終ノード） |
| `[[node]].branches` | list[str] | fork のみ | — | 並列に走らせるブランチのノード id |
| `[[node]].wait` | list[str] | join のみ | — | 完了を待つブランチ id |
| `[[node]].on_reject` | `{after=int, goto=str}` | 任意 | — | `after` 回 reject されたら `goto`（ノード id か `"__human__"`）へ。`node_aborted`（タイムアウト/予算/API/stuck）もこの方針に従う（`DESIGN.md` §16.1） |
| `[[node]].tools` | list[str] | 任意 | （常時 harness コマンドのみ） | このノードの worker に渡すツールセット |
| `[[node]].context` | `{include=list[str]}` | 任意 | `{include=["none"]}` | コード context 事前計算の指示。要素は `"outline:$blast_radius"` / `"body:$target_symbols"` / `"none"` の 3 つに確定（語彙の拡張は実装時） |
| `[[node]].artifact_tags` | list[`{tag=str, gates=list}`] | 任意 | `[]` | tag ごとに課す追加 gate |
| `[[node]].model` | str | 任意 | `[meta].default_model` | このノードの worker のモデル（`DESIGN.md` §16.1） |
| `[[node]].budget` | table | 任意 | `[meta].default_budget` | `{max_tool_calls, max_tokens, max_wall_seconds}`。超過で `node_aborted{reason:budget}`（`DESIGN.md` §16.1） |
| `[[node]].cmd_allowlist` | list[str] | 任意 | — | `run-command` ツールが受け付けるコマンドパターン。`cmd_exit_0` の gate コマンドは暗黙的に許可（`DESIGN.md` §16.2） |
| `[[node]].network` | bool | 任意 | `false` | `true` のときだけ worker にネットワークを許す（`DESIGN.md` §16.2） |

注: `context.include` のプレースホルダは 2 つに確定 ── `$blast_radius`（当該ノードの `serves` の F-NNN の `files` の和集合）と `$target_symbols`（そのうち編集対象として判明しているシンボル ── まだ意味が完全には定まっていない、確定は実装時）。harness がこれらを解決する（`docs/worker-context.md` B2 参照）。`tools` の語彙は `harness ... / read / edit / write / run-command / outline / show-symbol / find-symbol / refs / callers / implementers / deps / rdeps / closure / impacted-by / tested-by`（`docs/worker-context.md` B1）。

**append と mandatory_gates**: `workflow.toml` は実行中に append できるが、append できるのは `can_append = true` のノードだけで、許可されるのは「新規 `[[node]]` の追加」と「まだ到達していないノードの `next`/`branches`/`wait` への配線追加」のみ（既存ノード・既存 `exit_gates` の変更/削除/弱体化、`on_reject` の緩和、既存ノードへのツール追加、`context` の拡大、`[meta].entry` の変更は禁止）。新規 append されたノードは `[meta].mandatory_gates` に挙がった gate を（自分の `exit_gates` に）含まなければならない。これらを `workflow_append_only` gate が run 開始時の `workflow.toml`（or そのハッシュ）との差分に対して検証する（`DESIGN.md` §5.1）。

### 2.2 デフォルトワークフローの例（research/scope → plan → characterize → implement → test → security → review → done）

> 注: `harness init` が生成するノード集合の正典は `crates/harness-core/src/scaffold/workflow_tmpl.rs`。
> 現行 canonical は research → plan → design-pre → characterize → implement → test → **verify** → security
> → review → docdesign の 10 ノード。本例はその簡略版で design-pre / verify / docdesign を含まない ──
> ノード集合は workflow_tmpl.rs が優先する。verify は outside-in 観測ノードで、white-box gate を満たしても
> 現実(描画 / 本番固有データ / 仕様誤読)が壊れる型を実機観測で塞ぐ(`{verify}` cmd は任意、強制力は
> verify_observation evidence)。

これがデフォルトワークフロー（`DESIGN.md` §3・§5）。`characterize` は任意ノード（plan と implement の間に置く）── 影響ファイルのカバレッジが閾値未満なら fail する `cmd_exit_0` を出口 gate に持ち「変える前に characterization test を書け」を強制する（専用プリミティブは作らない、`DESIGN.md` §7）。不要なら省略してよい。`plan` ノードは plan-approval gate（人間チェックポイント 2 つ目、`DESIGN.md` §13）、`security` ノードは test の後・review の前に最終セキュリティ確認を担う（`docs/skill-templates.md` の `## skill: security`）。`implement` は分解されると `fork`/`join` で並列化される（§2.4）。

```toml
[meta]
name = "default-refactor-flow"
entry = "research"                          # 開始ノード id
host = "claude-code"                        # ホスト＝Claude Code（plan モード・/security-review・/review・AskUserQuestion を skill が優先利用）。runtime のときは "runtime"（DESIGN.md §10）
default_model = "opus"                      # ノードに model が無いとき適用。ティアエイリアス(opus/sonnet/haiku/fable)、具体 ID は resolve_model が解決（DESIGN.md §16.1）
run_cost_budget = 50.0                      # run 全体のコスト上限。超過で人間にエスカレ
# 実行中に append される新規ノードはこれらの gate を必ず含む（workflow_append_only が検証）
mandatory_gates = [
  { gate = "traceability_closed", args = {} },
  { gate = "cmd_exit_0", args = { cmd = "cargo check --workspace" } },          # ビルドが通る ── per-crate でなく workspace 全体（domain をまたぐ署名 break を導入ノードで安く捕まえる、DESIGN.md §16.1・operations.md §1）
  { gate = "cmd_exit_0", args = { cmd = "gitleaks detect --no-git --redact" } },  # ソースに書いたシークレットを捕まえる（DESIGN.md §16.2）
]

[[node]]
id = "research"
skill = "01-research.md"
exit_gates = [
  { gate = "evidence_recorded", args = { key = "research_notes" } },
  { gate = "open_questions_zero", args = {} },
  { gate = "no_pending_required_questions", args = {} },
  { gate = "json_has", args = { evidence_key = "human_approval", json_path = "verdict", eq = "approved" } },
  { gate = "blast_radius_declared", args = {} },
]
next = ["plan"]
on_reject = { after = 3, goto = "__human__" }   # 3回 reject で人間エスカレ
tools = ["read", "outline", "show-symbol", "find-symbol", "refs", "callers", "deps", "closure", "impacted-by", "tested-by"]
context = { include = ["none"] }            # 研究ノードは事前計算なし（worker が semantic クエリで探索）
# serves は省略（spec 自体をこのノードで作る）。artifact_tags も省略可

[[node]]
id = "plan"
skill = "02-plan.md"
can_append = true                           # plan ノードだけが workflow.toml を append できる（DESIGN.md §5.1）
serves = ["F-001", "F-002"]
exit_gates = [
  { gate = "artifact_registered", args = { name_or_prefix = "plan" } },
  { gate = "max_lines", args = { path = "plan.md", n = 200 } },
  { gate = "traceability_closed", args = {} },
  { gate = "workflow_append_only", args = {} },
  # plan-approval gate ── 人間チェックポイント 2 つ目（spec 承認に続く、DESIGN.md §13）
  { gate = "json_has", args = { evidence_key = "plan_approval", json_path = "verdict", eq = "approved" } },
]
next = ["characterize"]
on_reject = { after = 3, goto = "research" }     # 3回 reject で research へ戻る
tools = ["read", "outline", "show-symbol", "find-symbol", "deps", "closure", "impacted-by", "tested-by"]
context = { include = ["outline:$blast_radius"] }

# --- 任意ノード: 影響ファイルのカバレッジが閾値未満なら characterization test を先に書かせる ---
[[node]]
id = "characterize"
skill = "03-characterize.md"
serves = ["F-001", "F-002"]
exit_gates = [
  # カバレッジツールを blast radius に対し走らせ、閾値未満なら fail（カバレッジツール名・閾値はプロジェクト依存）
  { gate = "cmd_exit_0", args = { cmd = "<カバレッジツール> --affected <blast_radius> --fail-under 80" } },
]
next = ["implement"]
on_reject = { after = 3, goto = "plan" }
tools = ["read", "edit", "run-command", "outline", "show-symbol", "find-symbol", "tested-by"]
context = { include = ["outline:$blast_radius"] }

[[node]]
id = "implement"
skill = "04-implement.md"
serves = ["F-001", "F-002"]
budget = { max_tool_calls = 200, max_tokens = 400000, max_wall_seconds = 1800 }   # 超過で node_aborted{reason:budget}（DESIGN.md §16.1）
cmd_allowlist = ["cargo *", "git diff *"]   # run-command が受け付けるパターン。cmd_exit_0 の gate コマンドは暗黙的に許可（DESIGN.md §16.2）
exit_gates = [
  { gate = "artifact_registered", args = { name_or_prefix = "impl:" } },
  { gate = "no_regex", args = { path = "src/**/*.rs", pattern = "TODO|TBD|WIP|FIXME|未定|未確定|要検討|検討中|対応予定|サンプル|ダミー|仮置き" } },
  { gate = "cmd_exit_0", args = { cmd = "cargo test --lib", timeout_seconds = 600 } },
]
next = ["test"]
on_reject = { after = 3, goto = "plan" }
tools = ["read", "edit", "run-command", "outline", "show-symbol", "find-symbol", "refs"]
context = { include = ["outline:$blast_radius", "body:$target_symbols"] }
artifact_tags = [
  { tag = "new", gates = [ { gate = "max_lines", args = { n = 200 } } ] },          # 新規ファイルは 200 行以下
  { tag = "legacy", gates = [ { gate = "lines_not_increased", args = { baseline_key = "legacy_baseline" } } ] },  # レガシーは増やすな
]

[[node]]
id = "test"
skill = "05-test.md"
serves = ["F-001", "F-002"]
exit_gates = [
  { gate = "cmd_exit_0", args = { cmd = "cargo test --test '*'" } },
  { gate = "cmd_exit_0", args = { cmd = "./e2e.sh" } },
  { gate = "cmd_exit_0", args = { cmd = "cargo llvm-cov --fail-under-lines 95" } },
  { gate = "count_non_decreasing", args = { evidence_key = "tests_count", baseline_key = "tests_count_baseline" } },
  { gate = "evidence_recorded", args = { key = "test_result" } },
]
next = ["security"]
on_reject = { after = 3, goto = "implement" }
tools = ["read", "run-command", "edit"]
context = { include = ["none"] }

# --- security: 最終セキュリティ確認（test の後・review の前）。ホストに /security-review があれば skill がそれを invoke する ---
# skill ファイルは連番（01-research / 02-plan / 03-characterize / 04-implement / 05-test / 06-security / 07-review / 08-join）
[[node]]
id = "security"
skill = "06-security.md"
serves = ["F-001", "F-002"]
exit_gates = [
  { gate = "cmd_exit_0", args = { cmd = "cargo audit" } },                          # 依存脆弱性（DESIGN.md §16.2）
  { gate = "cmd_exit_0", args = { cmd = "gitleaks detect --no-git --redact" } },    # ソースに書いたシークレット
  { gate = "evidence_recorded", args = { key = "security_review" } },               # security findings の報告
  # 高リスク変更時（認証・決済・権限まわり）は次も足す ── verdict が clean か addressed であること
  # { gate = "json_has", args = { evidence_key = "security_review", json_path = "verdict", eq = "clean_or_addressed" } },
]
next = ["review"]
on_reject = { after = 3, goto = "implement" }
tools = ["read", "run-command", "outline", "show-symbol", "find-symbol", "refs"]
context = { include = ["outline:$blast_radius"] }

[[node]]
id = "review"
skill = "07-review.md"
serves = ["F-001", "F-002"]
exit_gates = [
  { gate = "json_has", args = { evidence_key = "review", json_path = "verdict", eq = "approved" } },
  { gate = "traceability_closed", args = {} },
]
next = []                                   # 最終ノード
on_reject = { after = 2, goto = "__human__" }
tools = ["read", "outline", "show-symbol"]
context = { include = ["outline:$blast_radius"] }
```

### 2.3 1 ノードだけの最小ワークフロー（小さな改修用）

```toml
[meta]
name = "tiny-fix"
entry = "fix"

[[node]]
id = "fix"
skill = "fix.md"
serves = ["F-001"]
exit_gates = [
  { gate = "artifact_registered", args = { name_or_prefix = "impl:" } },
  { gate = "no_regex", args = { path = "src/**/*.rs", pattern = "TODO|TBD|WIP|FIXME" } },
  { gate = "cmd_exit_0", args = { cmd = "cargo test" } },
]
next = []
on_reject = { after = 3, goto = "__human__" }
tools = ["read", "edit", "run-command", "show-symbol", "find-symbol", "refs"]
context = { include = ["outline:$blast_radius", "body:$target_symbols"] }
```

### 2.4 fork / join（並列ブランチ）

`fork` ノードが N 本の並列ブランチを spawn し、対応する `join` ノードが全ブランチ完了を待ってマージ・再検証する（`DESIGN.md` §11.1）。前提:

- **fork で並列化する前に `blast_radius_disjoint` が満たされていること** ── 各ブランチ（ここでは `impl_a` / `impl_b`）の宣言された影響ファイル集合に共通要素が無い（例では `impl_a` が `src/auth/*.rs`、`impl_b` が `src/session/*.rs`）。重なる場合は thin な選択として並列化を拒否する（保守的・決定論的）。
- **各並列ブランチは自分の sub-log を持つ**（`state/<run-id>.<branch>.jsonl`） ── 1 個の jsonl への並行書き込みロックを避ける。`join` ノードがこれらをマージする。
- **`join` の出口 gate にはマージ結果に対する結合/フルスイート再実行が必須** ── 個別に green なブランチが互いを壊しうるため（integration 問題）。

```toml
[meta]
name = "parallel-refactor-flow"
entry = "plan"

[[node]]
id = "plan"
skill = "02-plan.md"
can_append = true                           # plan ノードだけが workflow.toml を append できる（fork/join 追加もここ、DESIGN.md §5.1）
exit_gates = [
  { gate = "artifact_registered", args = { name_or_prefix = "plan" } },
  { gate = "traceability_closed", args = {} },
  { gate = "blast_radius_declared", args = {} },
  { gate = "workflow_append_only", args = {} },
]
next = ["split"]                            # plan が F-007 を F-007.1 / F-007.2 に分解し split へ
serves = ["F-007"]
tools = ["read", "outline", "show-symbol", "find-symbol", "deps", "closure"]
context = { include = ["outline:$blast_radius"] }

# --- fork ノード: 2 本の並列ブランチを spawn ---
[[node]]
id = "split"
type = "fork"                               # type 省略時は "task"。"fork" で並列ブランチ起動
# fork ノードは skill 任意（worker 作業がほぼ無い ── ブランチ起動の宣言だけ）。skill 行は付けない
branches = ["impl_a", "impl_b"]             # 並列に走らせるノード id（各々が自分の sub-log を持つ）
exit_gates = [
  { gate = "blast_radius_disjoint", args = { node_a = "impl_a", node_b = "impl_b" } },  # 互いに素でなければ並列化拒否
]
next = ["merge"]
tools = ["read"]
context = { include = ["none"] }

# --- ブランチ A: src/auth/ だけを触る（impl_b と互いに素）---
[[node]]
id = "impl_a"
skill = "04-implement.md"
serves = ["F-007.1"]
exit_gates = [
  { gate = "artifact_registered", args = { name_or_prefix = "impl:auth" } },
  { gate = "no_regex", args = { path = "src/auth/**/*.rs", pattern = "TODO|TBD|WIP|FIXME" } },
  { gate = "cmd_exit_0", args = { cmd = "cargo test auth::" } },
]
next = []                                   # join がブランチ完了を待つ
tools = ["read", "edit", "run-command", "show-symbol", "find-symbol", "refs"]
context = { include = ["outline:$blast_radius", "body:$target_symbols"] }

# --- ブランチ B: src/session/ だけを触る（impl_a と互いに素）---
[[node]]
id = "impl_b"
skill = "04-implement.md"
serves = ["F-007.2"]
exit_gates = [
  { gate = "artifact_registered", args = { name_or_prefix = "impl:session" } },
  { gate = "no_regex", args = { path = "src/session/**/*.rs", pattern = "TODO|TBD|WIP|FIXME" } },
  { gate = "cmd_exit_0", args = { cmd = "cargo test session::" } },
]
next = []
tools = ["read", "edit", "run-command", "show-symbol", "find-symbol", "refs"]
context = { include = ["outline:$blast_radius", "body:$target_symbols"] }

# --- join ノード: 全ブランチ完了を待ち、マージし、フルスイートで再検証 ---
[[node]]
id = "merge"
type = "join"                               # "join" で全ブランチ完了待ち＋sub-log マージ
skill = "08-join.md"                        # join は短い skill を持つ（再検証 worker が動く）
serves = ["F-007", "F-007.1", "F-007.2"]
wait = ["impl_a", "impl_b"]                 # この全ブランチが done になるまで待つ
exit_gates = [
  # マージ＋結合/フルスイート再実行が必須（個別 green が互いを壊しうるため）
  { gate = "cmd_exit_0", args = { cmd = "git merge --no-ff impl_a impl_b && cargo test" } },
  { gate = "cmd_exit_0", args = { cmd = "cargo test --test '*'" } },
  { gate = "traceability_closed", args = {} },           # 全 F-NNN にわたって閉じているか
  { gate = "count_non_decreasing", args = { evidence_key = "tests_count", baseline_key = "tests_count_baseline" } },
]
next = []
on_reject = { after = 2, goto = "__human__" }
tools = ["read", "run-command", "edit"]
context = { include = ["none"] }
```

## 3. gate プリミティブ・リファレンス（正典）

**この表が gate プリミティブの完全な引数・定義・戻り値の唯一の正典である**（`DESIGN.md` §7 は散文＋このセクションへのポインタを持つ）。各 gate は `(state) -> (ok: bool, note: String)` の純粋関数。`eval_gate(name, args, state)`。未知の名前は `ok=false`。gate は L1〜L4 の決定論的プリミティブのみ（L5＝LLM 判断 gate は禁止）。

| 名前 | 引数（args のキー） | 定義 | 戻り値 |
|---|---|---|---|
| `file_exists` | `path`（glob 文字列 1 個） | path が実在ファイル | `(ok, note)` |
| `file_nonempty` | `path`（glob 文字列 1 個） | path が実在ファイルかつ中身非空 | `(ok, note)` |
| `max_lines` | `path`（glob 文字列 1 個）, `n` | path の行数 ≤ n | `(ok, note)` |
| `lines_not_increased` | `path`（glob 文字列 1 個）, `baseline_key` | path の行数が baseline（evidence に記録された値）以下 | `(ok, note)` |
| `no_regex` | `path`（glob 文字列 1 個。複数 path をチェックしたいなら複数の `no_regex` gate を並べる、または glob で表現する。要確認: 複数 path 指定の構文を入れるかは実装時）, `pattern` | path にマッチするどのファイルのテキストにも pattern がマッチしない | `(ok, note)` |
| `cmd_exit_0` | `cmd`, `timeout_seconds?` | シェルコマンド cmd を **harness 自身が `request-transition` 時にその場で実行**して exit code が 0。**worker の `report-evidence test_result` 等は申告であって metrics/notes 用の補助 ── 信頼の源泉ではない**（worker が「通った」と嘘をついても harness が再実行するので無意味）。runtime がタイムアウトを適用（既定値、args の `timeout_seconds` で上書き可）。gate コマンドは `workflow.toml` に事前宣言済みなので暗黙的に cmd_allowlist 済み（`DESIGN.md` §7・§16.2・`docs/operations.md` §1） | `(ok, note)` |
| `git_clean` | `untracked_only?`（bool, 既定 false）, `ignore?`（`\|` 区切りの path パターン。glob は段数厳密で nested は `**/` が要る／glob メタ無しは前方一致） | `git status --porcelain` を harness 自身が実行し working tree が clean（L12 clean handoff の「artifacts 除去」次元）。`untracked_only=true` なら追跡済み変更（正当な実装 diff）は許し未追跡（`??`）残骸だけを fail にする。run が commit しない終端ノードではこちらを使う | `(ok, note)` |
| `json_has` | `evidence_key`, `json_path`, `eq?` | `gate_evidence[evidence_key]` が存在し `json_path` の値が存在（`eq` 指定時はその値と等しい） | `(ok, note)` |
| `artifact_registered` | `name_or_prefix` | その名前（または `impl:` のような prefix）の artifact が ≥1 件登録され、全て実在ファイル | `(ok, note)` |
| `evidence_recorded` | `key` | `gate_evidence[key]` が存在する | `(ok, note)` |
| `traceability_closed` | （なし） | 全 F-NNN に実在 artifact ≥1 と exit 0 する test ≥1 が紐づく／登録ソース artifact がどれかの F-NNN に紐づく（orphan 検出）。詳細は `DESIGN.md` §6 | `(ok, note)` |
| `workflow_append_only` | （なし） | run 開始時の `workflow.toml`（or そのハッシュ）との差分が「追加のみ」（新規 `[[node]]`・未到達ノードへの `next`/`branches`/`wait` 配線）に収まることを検証。許可/禁止の完全なリストと append 細則は `DESIGN.md` §5.1 | `(ok, note)` |
| `count_non_decreasing` | `evidence_key`, `baseline_key` | `gate_evidence[evidence_key]` の数値が baseline 以上 | `(ok, note)` |
| `open_questions_zero` | （なし） | `spec.toml` のどの `text` フィールドにも `??` が無い ＋ `[[open_question]]` 配列が空。詳細は `DESIGN.md` §6・§1.1 | `(ok, note)` |
| `blast_radius_declared` | （なし） | spec の各 F-NNN に「影響ファイル ≥1」が紐づいている | `(ok, note)` |
| `no_pending_required_questions` | （なし） | 質問キュー（`state/<run-id>.questions.jsonl`）に `required: true` で未回答のエントリが無い（`DESIGN.md` §13） | `(ok, note)` |
| `blast_radius_disjoint` | `node_a`, `node_b` | 2 つのノードの宣言された blast radius（影響ファイル集合）が共通要素を持たない。fork で並列化する前提条件（`DESIGN.md` §11.1） | `(ok, note)` |

注: `traceability_closed` / `workflow_append_only` / `open_questions_zero` / `blast_radius_declared` は harness 内プリミティブにする価値あり。characterization は専用プリミティブにせず、`characterize` ノード（implement の前）の出口を `cmd_exit_0`（カバレッジチェック）にして強制する（§2.2 の例、`DESIGN.md` §7）。

## 4. コマンド・リファレンス（正典）

**この表が harness コマンドの完全な引数・効果の唯一の正典である**（`DESIGN.md` §12 は 3 群への分類＋このセクションへのポインタを持つ）。コード知能クエリは harness の一級コマンドではなく `query <backend-subcommand>` 経由でバックエンドへ素通しする（下表の `query` 行）。CKG コマンド（`reindex` / `ckg-stale`）の完全な説明は `docs/ckg.md` §6 を正典とし、この表はそこへのポインタを持つ。

| コマンド | 引数 | 効果 | 状態を変えるか |
|---|---|---|---|
| `start "<intent>"` | intent, `[--worktree <path>]` | 新 run 開始、`start` イベント、status 出力。`--worktree <path>` を付けると以後その run の全 `cmd_exit_0`・編集はその作業ディレクトリ内で行われ、終了時に diff を取る（複数 run の並行実行に必須）。worktree 自体の作成/破棄は harness が所有しない ── 外部ツール（`git worktree` 等）の責任（`DESIGN.md` §11.2） | 変える |
| `status` | `[--run R]` | run_id, intent, 現ノード名と番号, skill 絶対パス, 出口 gate 一覧と pass/fail＋理由, artifacts, gate_evidence キー, done か | 変えない |
| `advance <to>`（別名 `request-transition`） | `to`, `[--run R]` | 現ノードの出口 gate を全評価、全 pass で `advance` イベント＋新 status、fail で `advance_rejected` イベント＋fail 一覧＋exit 1 | 変える（reject も記録） |
| `back "<reason>"` | `reason`, `[--run R]` | 前ノードへ、`back` イベント | 変える |
| `record-artifact <name> <path>` | `name`, `path`, `[--tag T]`, `[--run R]` | path 実在を確認、`artifact` イベント | 変える |
| `report-evidence <gate> <json\|@file>` | `gate`, `json`, `[--run R]` | json をパース、`gate_evidence` イベント | 変える |
| `ask "<質問>"` | `質問`, `--option ...`（2〜4 個）, `[--required]`, `[--run R]` | worker 向け。構造化質問を質問キューに積む（`question_queued` イベント。`required` 指定時は `no_pending_required_questions` gate がノードをブロック。`DESIGN.md` §13） | 変える |
| `questions` | `[--run R]` | 人間向け。保留中の質問（未回答エントリ）を一覧 | 変えない |
| `answer <question-id> <選択肢index\|"自由記述">` | `question-id`, `回答`, `[--run R]` | 人間向け。回答 → `human_answer` イベント。`kind=clarification` なら `spec.toml` の該当箇所を更新し `??` をクリア（`DESIGN.md` §13） | 変える |
| `reset` | `[--run R] --yes` | `reset` イベント | 変える |
| `skill` | `[--run R]` | 現ノードの skill 内容/パス | 変えない |
| `spec <F-NNN>` | `F-NNN` | その要件と AC と紐づくテストだけ | 変えない |
| `gates` | `[--run R]` | 保留 gate 各 1 行 | 変えない |
| `query <backend-subcommand> [args]` | `subcommand`, `[args]` | 設定されたコード知能バックエンドへの素通し。一覧はバックエンド依存（典型: `find-symbol` / `refs` / `callers` / `implementers` / `show-symbol` / `outline` / `deps` / `rdeps` / `closure` / `impacted-by` / `tested-by`）。`DESIGN.md` §9・§12 | 変えない（バックエンド委譲） |
| `reindex [--full]` | `[--full]` | 外部索引器を叩いて CKG キャッシュ artifact を再生成（インクリメンタル＝変更ファイル＋逆依存閉包のみ。`--full` で全体を再構築。atomic swap で並行読みと干渉しない）。完全な仕様は `docs/ckg.md` §6 | 変えない（キャッシュ artifact 更新） |
| `ckg-stale` | （なし） | CKG が git HEAD に対し陳腐化しているか（陳腐ファイル一覧）。完全な仕様は `docs/ckg.md` §6 | 変えない |
| `init` | `[--workflow path]`, `[--spec path]`, `[--host claude-code\|runtime]` | onboarding スキャフォールド ── 既存 repo に `workflow.toml` / `spec.toml` のひな型と `skills/` を置き、内部で `harness validate` を実行し、スモークチェック（gate コマンドが解決するか・skill ファイルが揃っているか等）を行う。詳細は `docs/onboarding.md` | 変える（ファイル生成） |
| `doctor` | `[--workflow path]`, `[--spec path]` | スモークチェックを再実行し、config / skill / ツール設定のドリフト（参照先欠落・gate コマンド未解決・skill 不在等）を flag する。詳細は `docs/onboarding.md` | 変えない |
| `validate` | `[--workflow path]`, `[--spec path]` | `workflow.toml` / `spec.toml` の静的検証（next/branches/wait の参照先・`[meta].entry`・サイクル方針［`next` で前方サイクルを作るのは error。前ノードへ戻れるのは `back` / `on_reject` の `goto` 経由のみ］・gate 名と args・`serves` の F-ID・`skill` ファイル実在・到達可能&停止・`mandatory_gates` 妥当）。`harness start` 時に自動実行（`DESIGN.md` §16.4） | 変えない |
| `inspect <run-id>` | `run-id`, `[--node X]` | ノードの状態・gate・artifact・transcript への参照を見る（`DESIGN.md` §16.3） | 変えない |
| `replay <run-id>` | `run-id` | イベントログを頭から fold して状態遷移を再現（`DESIGN.md` §16.3） | 変えない |
| `stats <run-id>` | `run-id` | ノードごとの context トークン数・コスト・tool-call 数（圧縮効果の計測、`DESIGN.md` §16.3） | 変えない |
| `stuck "<理由>"` | `理由`, `[--run R]` | worker 向け。これ以上進めないと自己申告 → `node_aborted{reason:stuck}` ＋ エスカレ（`DESIGN.md` §16.1） | 変える |
| `abandon <run-id>` | `run-id`, `[--reason R]` | run を放棄する ── `abandon` イベント（理由を payload に）を書く（イベントログが SSOT）。FS 上の worktree の後始末（`--worktree` モードなら `git worktree remove`、そうでなければ `git reset`）はイベントとは別の外部作業（`DESIGN.md` §16.5） | 変える |

## 5. イベント種別・リファレンス（正典）

**この表がイベント payload の唯一の正典である**（`DESIGN.md` §4 は状態モデルの説明上イベント名＋1 行の説明だけを持ち、payload はここを参照する）。jsonl、各行 1 JSON、共通フィールド `ts`（ISO8601 UTC）。`derive_state(events) -> State` は純粋 fold。

| type | payload フィールド | いつ書かれるか |
|---|---|---|
| `start` | `intent` | `start` コマンド時。run の最初のイベント |
| `advance` | `from`, `to` | `request-transition` / `advance` で出口 gate が全 pass したとき。phase_index +1。ノード完了時のメトリクスはこのイベントには載せず、サイドカー `state/<run-id>.metrics.jsonl` に書く（下記注、`DESIGN.md` §16.1） |
| `advance_rejected` | `failed_gates: [{gate, reason}]` | `request-transition` / `advance` で 1 つでも gate が fail したとき（記録のみ、状態の phase は進まない） |
| `back` | `reason` | `back` コマンド時。phase_index を saturating -1 |
| `artifact` | `name`, `path`, `tag?` | `record-artifact` 時。path 実在確認後。同名は上書き |
| `gate_evidence` | `gate`, `data` | `report-evidence` 時。json パース後。同 gate キーは上書き |
| `reset` | （なし） | `reset --yes` 時。以降のイベントだけで再構築（run_id/intent は最初の start から保持） |
| `node_appended` | `node_def` | plan ノード等が `workflow.toml` にノードを追加したとき |
| `question_queued` | `question: {id, kind, header, question, options, required, context_ref}` | worker が `harness ask` で構造化質問を質問キューに積んだとき（`DESIGN.md` §13） |
| `human_answer` | `question_id`, `answer` | 人間が `harness answer` で回答したとき。`kind=escalation` の回答は従来の `human_decision` を兼ねる（`human_decision` は `human_answer`(kind=escalation) に統合した、`DESIGN.md` §4） |
| `branch_forked` | `branch_ids` | fork ノードが並列ブランチを開始したとき。各ブランチは自分のイベントを `state/<run-id>.<branch>.jsonl` に書く |
| `branch_joined` | `branch_ids`, `merge_result` | join ノードが全ブランチをマージし検証したとき |
| `node_aborted` | `node`, `reason` (`"timeout"`\|`"budget"`\|`"api_error"`\|`"stuck"`\|`"crash"`) | gate タイムアウト・ノード予算超過・API リトライ尽き・worker の `harness stuck`・クラッシュ復旧時の中途ノード破棄。当該ノードの `on_reject` に従う（`DESIGN.md` §16.1） |
| `abandon` | `reason` | `abandon <run-id>` 時。run を放棄状態にする（terminal）。worktree の後始末はイベントとは別（`DESIGN.md` §16.5） |

注: 並列ブランチの sub-log は `<run-id>.<branch>.jsonl` という命名規約（例: run-id が `r123` でブランチ `impl_a` なら `state/r123.impl_a.jsonl`）。join 時に親 run のログにマージされる（`DESIGN.md` §11.1）。

注（メトリクス・サイドカー）: ノード完了時のメトリクス（`cost`, `tokens`, `tool_calls`, `wall_seconds`）はイベントログには載せず、append-only サイドカー `state/<run-id>.metrics.jsonl` に書く（各行 1 ノード分の `{node, cost, tokens, tool_calls, wall_seconds, ts}`）。`harness stats <run-id>` はこのサイドカーを読む（`DESIGN.md` §16.1・§16.3）。イベントログを軽く保つための分離。
