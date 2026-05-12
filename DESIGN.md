# DESIGN.md — thin-workflow-harness 設計方針

> 本ドキュメントは長い設計対話の結晶化である。`docs/schemas.md` と合わせて読むこと。
> 現在の `src/*.rs` は v0 prototype（5 フェーズをハードコードした版）であり、本ドキュメントの方向に作り直す予定（§15）。

## 1. これは何か / 目的

軽量ワークフローハーネス。**巨大な既存コードベース（10M ステップ級）への改修を、十分な上流壁打ちの後、指示通り一発で実装させる**ことが狙い。新規に巨大プログラムを生成するのが目的ではない。

同時に満たすべき要求:

1. コードと設計書の同期を決定論的に取る
2. 圧倒的に少ない context でそれを実現する
3. 単体・結合・E2E・リグレッションテストが自動で担保される

「一発で」の意味は「エージェントがノード内で 1 回も試行錯誤しない」ではない。**間違った中間状態が一度も下流に伝播せず、一度も "done" として確定しない**ということ。詳細は §4, §6。

## 2. 設計思想

- **thin harness / fat skills / fat code / fat data**（Garry Tan の "Thin Harness, Fat Skills, Fat Code" 論に基づく）。harness は薄いルーター。判断を要する曖昧な操作 → markdown skill（fat skills、引数化必須・シナリオをハードコードしない）。完璧であるべき決定的処理 → コード（fat code、harness の外。harness は `cmd_exit_0` で呼ぶだけ）。コードベースの知識 → 事前蒸留（fat data、§9）。
- **状態は LLM に持たせない**。状態はコードが所有する型付きレコード。LLM は「遷移リクエスト」と「根拠」しか出さない。検証して状態を書くのは harness。
- **ワークフローはコードでなくデータ**。harness は gate 付きルーターであってワークフローエンジンではない（30 フェーズの重い harness も、5 フェーズに減らしただけのものも、同じ病気＝harness が太る）。
- **L1〜L4 の決定論的 gate のみ**。L5（LLM 判断）gate は禁止。それを入れた瞬間に決定論性が壊れる。
- **人間の判断は 1 点に集約**: 「この spec は俺が欲しかった変更か」（spec 承認）＋ 壁打ち中の質問への回答 ＋ 想定外のエスカレ。それ以外は機械的。人間のレビュー負荷は O(spec) であって O(diff) ではない（spec は変更が巨大でも小さい）。
- **再発する変更パターンは playbook 化して再利用する（複利）**: 同種の改修が繰り返されるなら、その手順を skill（playbook）に蒸留して次回以降に効かせる。仕組みは opt-in の retrospective ノード＋playbook（skillify）。詳細は `docs/skillify.md`、配線箇所は §5。

## 3. アーキテクチャ概観

デフォルトワークフロー（`docs/schemas.md` §2.2 の線形例の素体）は **research/scope → plan → characterize → implement(×N, fork/join) → test → security → review → done**。`plan` ノードは plan-approval gate（人間チェックポイント 2 つ目 ── spec 承認に続く）、`security` ノードは test の後・review の前に最終セキュリティ確認を担う。harness 自身はこの並びを知らない（`workflow.toml` のデータ）。

harness が**所有するもの**（薄いまま、ほぼ未来永劫変わらない）:

1. イベントログ（append-only jsonl）+ `derive_state`（純粋 fold）
2. ループ / コマンド表面（状態機械系・問い合わせ系・運用系 ── 完全な一覧と引数・効果は §12 ＝ 散文、正典は `docs/schemas.md` §4）
3. プリミティブ gate 評価器 ~16 個前後（汎用・意見ゼロ・config で引数化）（§7、正典は `docs/schemas.md` §3）
4. config ローダー: `workflow.toml`（ノード/エッジ/出口 gate）＋ `spec.toml`（F-NNN/AC-N/トレースマップ、optional）
5. ノードごとに worker を spawn し、その context を組み立てるランタイム（§10）

harness が**持たないもの**:

- フェーズの名前（research/plan/… は `workflow.toml` に書く）
- 「200」という数字、禁止語リスト（`workflow.toml` で `max_lines` / `no_regex` の引数として渡す）
- プロジェクト固有ロジック（それは fat code。`cmd_exit_0` で呼ぶだけ）
- LLM 判断 gate
- フル索引器（コード知能は外部に委譲、§9・§12）
- worktree のライフサイクル管理（作成/破棄/隔離は外部ツールの責任、§11.2）
- 頂点の LLM オーケストレーター（オーケストレーションは決定論的、§10）

```
harness バイナリ（thin, 変わらない）
  ├ event log + derive_state
  ├ loop / コマンド（状態機械系 ＋ 問い合わせ系 ＋ 運用系 ── 一覧は §12、正典は docs/schemas.md §4）
  ├ プリミティブ gate ~16種（汎用・意見ゼロ ── 一覧は §7、正典は docs/schemas.md §3）
  ├ config ローダー（workflow.toml + spec.toml）
  └ worker ランタイム（ノードごとに生APIで spawn、context を harness が構築）
workflow.toml（データ、編集可能）── プロセス: ノード/エッジ/出口gate。実行中に append できる
spec.toml（データ、optional）── 設計書の検証可能部分: F-NNN / AC-N(各々に testコマンド) / トレースマップ / 不変条件 / open_questions
skills/*.md（markdown, fat）── 各ノードで何を作るか / record-artifact・report-evidence の使い方
コード知能バックエンド（外部、プラガブル）── LSP / SCIP/LSIF 索引 / tree-sitter
fat code（外部）── プロジェクトのテスト・linter・coverage・IF署名チェック ── cmd_exit_0 で呼ばれるだけ
```

並列実行（fork/join ノード、複数 run の同時実行、worktree 隔離）については §11 参照。playbook / skillify（再発パターンの再利用）は §2・§5 と `docs/skillify.md`、コードナレッジグラフ（上図「コード知能バックエンド」の実体）は `docs/ckg.md` を参照。運用面（resilience / 予算・セキュリティ / 信頼境界・可観測性 / デバッグ・config 検証・deliverable ライフサイクル）は §16 と `docs/operations.md`。コマンド表面の完全な一覧・gate プリミティブの完全な引数定義・イベント payload は `docs/schemas.md`（§3 gate / §4 コマンド / §5 イベント）が唯一の正典 ── 本ドキュメントは散文＋ポインタを持つ。

## 4. 状態モデル

- **append-only イベントログ（jsonl、`$HARNESS_HOME/state/<run-id>.jsonl`）が SSOT**。各行 1 JSON、共通フィールド `ts`（ISO8601 UTC）。
- イベント種別（名前＋1 行。**完全な payload は `docs/schemas.md` §5 が正典**）:
  - `start` ── run の最初のイベント、変更依頼の intent を記録
  - `advance` ── 出口 gate が全 pass し次ノードへ遷移（phase_index +1）。ノード完了時のメトリクスはこのイベントには載せず、サイドカー `state/<run-id>.metrics.jsonl` に書く（§16.1）
  - `advance_rejected` ── 出口 gate が 1 つでも fail（記録のみ、phase は進まない）
  - `back` ── 前ノードへ戻る（phase_index を saturating -1）
  - `artifact` ── `record-artifact` で artifact を登録（path 実在確認後、同名上書き）
  - `gate_evidence` ── `report-evidence` で gate の根拠 JSON を記録（同 gate キー上書き）
  - `reset` ── 以降のイベントだけで状態を再構築（run_id/intent は最初の start から保持）
  - `node_appended` ── plan ノード等が `workflow.toml` にノードを追加（ワークフローがデータとして実行中に成長）
  - `question_queued` ── worker が `harness ask` で構造化質問を質問キューに積んだ（§13）
  - `human_answer` ── 人間が `harness answer` で回答（`kind=escalation` の回答は従来の `human_decision` を兼ねる ── `human_decision` は `human_answer`(kind=escalation) に統合した）
  - `branch_forked` ── fork ノードが並列ブランチを開始（各ブランチは自分のイベントを `state/<run-id>.<branch>.jsonl` に書く、§11.1）
  - `branch_joined` ── join ノードが全ブランチをマージし検証（§11.1）
  - `node_aborted` ── gate タイムアウト・ノード予算超過・API リトライ尽き・worker の `harness stuck`・クラッシュ復旧時の中途ノード破棄。書かれたら当該ノードの `on_reject` に従う（§16.1）
  - `abandon` ── `harness abandon <run-id>` で run を放棄（terminal）。payload `{reason}`。worktree の後始末はイベントとは別（§16.5）
- `derive_state(events) -> State`: 純粋 fold。同じイベント列は必ず同じ状態 ＝ 文字通り決定論的。`reset` が来たら「それ以降のイベントだけ」で再構築（ただし `run_id` / `intent` は最初の `start` から保持）。`advance` で phase_index +1、`back` で saturating -1、`artifact` / `gate_evidence` で map 更新（同名上書き）。**クラッシュ復旧**: harness が死んでも、最後にコミットされたイベントから derive して再開できる（中途ノードは fresh worker で再 spawn、§16.1）。
- **LLM は write-state できない**。`record-artifact name path` / `report-evidence gate json` で「リクエスト＋根拠」を出すだけ。harness が path の実在を確認し、json をパースし、その上でイベントを書く（自己申告を信じない）。`write-state` も `skip` もコマンドに存在しない ── これが thin harness の安全性そのもの。
- セッション / worker をまたいで状態を読み直すときは必ずイベントログから derive する。LLM の context に状態を持ち越さない。

## 5. ワークフローモデル

- `workflow.toml` がノードを定義: `id`, `skill`（`skills/` 配下のファイル名）, `exit_gates`（プリミティブ名＋引数のリスト）, `next`（次ノード id、複数候補可）, `on_reject`（N 回 reject されたら遷移する先 ── リトライ/エスカレ方針を*ここに書く*）, `tools`（このノードの worker に渡すツールのリスト ── ノードごとツールスコープ）, `artifact_tags`（このノードで登録する artifact の tag と、tag ごとに課す追加 gate）。
- ノードはさらに任意で `model`（このノードの worker のモデル ── 重いノードは大きいモデル、§16.1）, `budget`（`{max_tool_calls, max_tokens, max_wall_seconds}` ── 超過で `node_aborted{reason:budget}`, §16.1）, `cmd_allowlist`（`run-command` ツールが受け付けるコマンドパターンのリスト ── `cmd_exit_0` の gate コマンドは暗黙的に許可, §16.2）, `network`（既定 `false`、`true` のときだけ worker にネットワークを許す, §16.2）を持てる。
- `[meta]` は `default_model` / `default_budget`（ノードに無いとき適用）/ `run_cost_budget`（run 全体のコスト上限、超過で人間にエスカレ, §16.1）/ `secrets_glob`（worker の context に入れないファイル, §16.2）/ `host`（`"claude-code" | "runtime"` ── ホストが何を組込みで提供するか、§10・`docs/host-capabilities.md`）を持てる。`[meta].mandatory_gates` の有力候補は `{ gate = "cmd_exit_0", args = { cmd = "cargo check --workspace" } }`（**workspace 全体** ── per-crate でなく workspace 全体にすることで domain をまたぐ署名 break をそれを導入したノードで安く捕まえる、§16.1・§17・`docs/operations.md` §1/§2/§6）＋ `{ gate = "cmd_exit_0", args = { cmd = "gitleaks detect --no-git --redact" } }`（エージェントがソースにシークレットを書いていないか、§16.2）。
- ノードは**1 個でも、線形（research→plan→characterize→implement→test→security→review）でも、DAG でも自由**。「research/plan/…」は 1 つの `workflow.toml` の中身であって harness の概念ではない。デフォルトワークフローは **research/scope → plan → characterize → implement(×N, fork/join) → test → security → review → done**（`plan` ノードに plan-approval gate、`security` ノードは test の後・review の前、`docs/schemas.md` §2.2）。
- **ワークフローは実行中に*追加*できる**: plan ノードの skill が「F-007 を `spec.toml` で F-007.1 / .2 に分解し、対応するノードを `workflow.toml` に追加せよ」と指示 → harness は `node_appended` イベントを記録しつつ追加ノードを実行し続ける（ワークフローがコードでなくデータとして実行中に成長する）。
- **`workflow_append_only` gate**: `workflow.toml` の run 開始時点との diff が「追加のみ」であることを検証。エージェントが既存ノードの gate を削除/弱体化できない。`workflow.toml` は config（run state＝イベントログではない）なので「LLM は状態を書かない」原則は崩れないが、書ける範囲を append のみに gate で縛る。

### 5.1 workflow.toml の append 細則

- ノードが `workflow.toml` を append できるのは、そのノードに **`can_append = true`** が付いている場合のみ（既定 `false`、通常は plan ノードだけに付ける）。
- **許可されること**:
  - 新規 `[[node]]` の追加
  - まだ到達していないノードの `next` / `branches` / `wait` への配線追加（既存ノードへの配線追加だが、そのノードに**未到達である限り**）
- **禁止されること**（`workflow_append_only` gate が強制）:
  - 既存 `[[node]]` の変更 / 削除
  - 既存 `exit_gates` の削除・弱体化（追加は可）
  - `on_reject` の緩和（厳格化は可）
  - 既存ノードへのツール**追加**（縮小＝narrowing のみ可）
  - `context` の拡大（縮小は可）
  - `[meta].entry` の変更
- 新規 append されたノードは **`[meta].mandatory_gates`**（`workflow.toml` の新フィールド ── 全ノード、または終端ノードが含むべき gate spec のリスト、`docs/schemas.md` §2.1）を満たさなければならない。
- 実装上は: run 開始時の `workflow.toml` の内容（or そのハッシュ）を最初のイベント（`start`）に記録し、`workflow_append_only` は現 `workflow.toml` との差分が上記の許可範囲に収まるかを検証する。

### 5.2 retrospective / playbook ノード（skillify）

再発する変更パターンを skill 化して再利用する（複利）ため、ワークフローの終端に opt-in の retrospective ノードを置ける（ほぼデータ＋1 ノードで足りる）。詳細設計（playbook の表現・「再発」検出ヒューリスティック・配線）は `docs/skillify.md`、設計思想は §2 を参照。

## 6. spec モデル（設計書とコードの同期）

- `spec.toml`（設計書の*検証可能な*部分）:
  - `F-NNN`（要件、1 行〜数行）
  - `AC-N`（受入基準、各々に「それを検証する test コマンド」を紐づける）
  - **トレースマップ**（`F-NNN` → 影響ファイル/artifact 名のリスト → test コマンドのリスト）
  - `invariant`（改修で維持すべき不変条件、これも AC として test 化）
  - `open_question`（`[[open_question]]` 配列。壁打ちの未解決点。`??` マーカーで本文中に書いてもよい）
- **spec は壁打ちで作る**: research/spec ノードの skill が壁打ちを駆動 ──「`spec.toml` を作れ — F-NNN、AC-N（各々に test コマンド）、影響ファイル一覧、不変条件一覧。`??` で未解決点を書き、人間に質問して全部潰せ。最後に人間の承認を取れ」。
- **spec 凍結の終了条件 = `open_questions_zero` gate（`??` が無い ＋ `[[open_question]]` 配列が空）＋ `json_has` gate（`human_approval` evidence の `verdict` が `approved`）**。これを満たさないと implement に進めない。＝壁打ちは「細かくやれ」だが、終わりが gate で定義される（ダラダラやらない）。
- **`traceability_closed` gate** が乖離を双方向に検出: すべての `F-NNN` に「実在する artifact が ≥1」「exit 0 する test コマンドが ≥1」が紐づいているか／登録されたソース artifact がどれか 1 つの `F-NNN` に紐づいているか（orphan code 検出）。spec を変えた（F-NNN 追加）→ この gate が落ちる → テストと artifact を紐づけるまで先に進めない。コードを足した → F-NNN に紐づく artifact として登録しないと orphan で落ちる。
- **限界**: 決定論的に同期できるのは設計書の*構造化部分*（要件・AC・インターフェース署名・ファイル inventory）だけ。`spec.toml` に「module X は `foo(a,b)->c` を export」と書けばパースして実コードと突き合わせる gate（`cmd_exit_0` で署名チェックスクリプト）は作れる。**散文の設計意図や図はダメ** ── できるのは「図ファイルの mtime < コード mtime → stale flag」程度。だから「検証可能なものは `spec.toml` に構造化して書け、散文は最小限」というルールになる。

## 7. gate プリミティブ

各 gate は `(state) -> (ok: bool, note: String)`（fail なら note=理由、pass なら一言）の純粋関数。`eval_gate(name, args, state)` で評価。未知の名前は `ok=false`。汎用・意見ゼロ・config で引数化。

gate は **L1〜L4 の決定論的プリミティブのみ**（L5＝LLM 判断 gate は禁止 ── 入れた瞬間に決定論性が壊れる）。プリミティブは ~16 個: `file_exists` / `file_nonempty` / `max_lines` / `lines_not_increased` / `no_regex` / `cmd_exit_0` / `json_has` / `artifact_registered` / `evidence_recorded` / `traceability_closed` / `workflow_append_only` / `count_non_decreasing` / `open_questions_zero` / `blast_radius_declared` / `blast_radius_disjoint` / `no_pending_required_questions`。**完全な引数・定義・戻り値は `docs/schemas.md` §3 が唯一の正典**（`traceability_closed` は §6、`workflow_append_only` の許可/禁止リストは §5.1、`open_questions_zero` は §6 をそれぞれ参照）。

**名前付き合成 gate（"implement の出口" のような塊）は `workflow.toml` / `spec.toml` 側に書く。harness 内はプリミティブのみ。**

**`cmd_exit_0` ── harness 自身がコマンドを実行するのが gate である。** `request-transition` 時に harness が*その場で*コマンドを走らせ、*その*exit code を使う。worker の `report-evidence test_result '{...}'` は申告であって metrics / notes 用の補助であり、信頼の源泉ではない ── worker が「テストが通った」と嘘をついても harness が再実行するので無意味（同じことが `gitleaks` / `cargo audit` 等の security gate にも当てはまる）。だから「テスト層の自動担保」が嘘をつけないのは、harness 自身がコマンドを走らせるから（§8 末尾、`docs/operations.md` §1）。

**`[meta].mandatory_gates` の有力候補（§5・§16.1・`docs/operations.md` §1/§2/§6）**:
- **workspace 全体の `cargo check`**（per-crate でなく ── domain をまたぐ署名 break を、それを導入したノードで安く捕まえる。per-crate だと壊した crate を呼ぶ別 crate のビルド失敗を後段まで見逃す）
- **`gitleaks` / `trufflehog` 系**（エージェントが*ソースに*書いたシークレットを捕まえる ── context に入れたものが API に渡る分は捕まえられない、§14）

**test / join ノードの gate は blast radius の言語/パッケージから導出すべき**（一度ハードコードしない）。Rust+TS をまたぐ改修なら `cargo nextest && pnpm test`。plan が分解して各 sub-requirement の blast radius を宣言するとき、harness/plan が触れたパッケージごとに test gate も設定する。あるいは常にフルスイートを回す（シンプルだが遅い）。どちらにするかは `[meta]` or onboarding で決める（要確認 ── 自動導出を入れるか手書きかは実装時）。詳細は `docs/operations.md` §1。

**characterization について**: 「変える前に characterization test を書け」は専用プリミティブ（旧 `characterize_before_change`）にはしない。代わりに、implement の前に `characterize` ノードを置き、その出口 gate を `cmd_exit_0`（カバレッジツールを影響ファイルに対し走らせ、閾値未満なら fail）にすることで強制する。例: `{ gate = "cmd_exit_0", args = { cmd = "<カバレッジツール> --affected <blast_radius> --fail-under 80" } }`（`docs/schemas.md` §2 の `characterize` ノード例参照）。

## 8. テスト層の自動担保

- 単体 / 結合 / E2E / カバレッジ = それぞれ `cmd_exit_0 <command>` gate を該当ノードの `exit_gates` に配線するだけ。harness はテストを特別扱いしない。例: implement ノード出口に `cmd_exit_0 "cargo test --lib"`、integration ノードに `cmd_exit_0 "cargo test --test '*'"`、e2e ノードに `cmd_exit_0 "./e2e.sh"`、`cmd_exit_0 "cargo llvm-cov --fail-under-lines 95"`。
- **速い gate / 遅い gate の二段**: implement ノード出口 = 速い gate（影響ファイルをカバーするテストだけ、数分）。別の最終ノード = 遅い gate（フルスイート、数時間でも可、CI に投げて非同期でも harness は exit code を待つだけ）。
- **リグレッション**: 「全スイート green」＋「`count_non_decreasing` でテスト数が縮んでない」＋ skill ルール「バグ修正は必ず regression test を 1 本追加」。"以前通ってたテストが今も通る" ＝ "全スイート green" なので、本質はスイートが縮まないことの保証。
- **カバレッジ未達時**: gate が落ちる → skill が「未カバーの影響行に characterization test / 単体テストを書け」と指示 → *生成*は LLM の仕事（fat skill）、*強制*は gate（thin harness）。
- 「自動で担保」の意味 = 該当ノードを通過するには該当テストコマンドが exit 0 でなければ物理的に進めない、しかも harness 自身がコマンドを走らせる（worker の `report-evidence test_result` は申告であって信頼の源泉でない、§7 の `cmd_exit_0` 注記）ので嘘がつけない。これが §6 の traceability と組み合わさると「要件を足したら、その要件に紐づくテストが green でないと先へ進めない」になる。

## 9. context 圧縮戦略

**食うものの分類**:

| # | 食うもの | 備考 |
|---|---|---|
| ① | コード読解 | 巨大 repo で最大 |
| ② | 検索・ツール出力 | |
| ③ | 状態・進捗 | イベントログ問い合わせで対処済 |
| ④ | skill・指示 | ノード単位 on-demand 注入で対処済 |
| ⑤ | spec・要件 | スコープしたスライスで対処済 |
| ⑥ | gate フィードバック | 簡潔構造化で対処済 |
| ⑦ | 会話履歴・スクラッチ | 壁打ち→spec→忘却で対処済 |

①②が本節の本題。

- **grep をやめて semantic（LSP / 索引）に寄せる**: grep はテキストマッチ＝偽陽性だらけ（コメント・文字列リテラル・同名の無関係識別子）、構造的な問い（定義は？呼んでるのは？型の中身は？実装してるのは？）に答えられず、切り分けにコードを読む羽目→context 爆発。LSP（`textDocument/definition`, `references`, `documentSymbol`, `workspaceSymbol`, `callHierarchy`, `typeHierarchy`, `implementation`）は構造的な問いに**厳密**に答え（偽陽性なし）、索引が全体を知ってるので**網羅的**（漏れない）、返るのは**位置（file:line, シンボル種別）であって本体ではない**ので、読むべき少数だけ本体を読む＝安い。この repo は既に Serena（LSP ラッパ）+ code-search-policy（Serena 優先、grep は純粋テキストパターンのみ）を使っており、新 harness もこの方針を内蔵する。grep は「ログ文字列」「リテラル定数」みたいな真のテキスト検索だけ許可。
- **シンボル単位で読む（ファイル単位で読まない）＋ アウトライン**: `show-symbol <sym>` ＝ その関数/型の本体だけ（LSP の `documentSymbol` レンジ）。800 行ファイルでなく 30 行の関数を読む。`outline <file>` ＝ シグネチャ＋docstring だけのスケルトン、800 行が 40 行に。「形を把握したい」はこれで足りる。
- **コードナレッジグラフ（CKG）**（詳細設計は `docs/ckg.md` 参照）: ノード=シンボル（関数・型・モジュール・ファイル）、エッジ=関係（defines/calls/imports/implements/references/contained-in/tested-by）。LSP か SCIP/LSIF（Sourcegraph / LSP の索引フォーマット）か tree-sitter から構築。構築済みなら「正しい範囲の特定」が検索でなくグラフクエリになる: `closure <entrypoint> --depth 2`（推移閉包＝blast radius 候補）, `impacted-by <sym>`（変えたら壊れうる箇所＝references エッジ）, `tested-by <sym>`（カバーするテスト＝速いリグレッション gate の対象）。**重要: グラフはコードを読まずに構造的な問いに答える**（「A は B に依存？」→グラフ参照、context にコードゼロ）。本体を読むのは実際に書き換える数シンボルだけ。グラフは `traceability_closed` gate にも効く（どのファイルがどのシンボルを実装し、どのテストがどのシンボルを参照するかをグラフが知ってる）。
- **パッケージカード（fat data）**: 大モジュールに「何をする/主要 export」の 1 段落要約を一度生成してキャッシュ、モジュール全体の代わりに食わせる。ドリフトリスク→グラフが「モジュールのシンボルが変わった」を教えるので再生成トリガにする or 「要約 mtime < コード mtime → stale flag」。
- **会話の外部化**: 壁打ちで 1 点決まったら即 `spec.toml` に書く→context から落とす。`harness spec <F>` で取り戻せる。context ＝ ワーキングセットであって履歴ではない。skill で「決めたら spec に書け、書いたら参照に切り替えろ」と強制。
- **context バジェット目標**: 任意の瞬間、worker の context ≒
  - 現ノードの skill（~30-50 行）
  - 現 spec スライス: F-NNN＋AC＋不変条件＋blast radius のファイル一覧（~20-40 行）
  - コンパクト status: ノード＋保留 gate 各 1 行（~10 行）
  - 今編集してるシンボルの本体（数百行、blast radius で上限）
  - その直接依存のアウトライン・シグネチャだけ（数十行）
  - 直近の gate フィードバック（~10-30 行）

  それ以外（残り 10M 行、会話履歴、他の 49 個の F-NNN、フルコールグラフ）は全部 context 外、`harness <query>` で取れる。**コードが context に入るのは (a) オリエンテーション用のアウトライン (b) 実際に触る数シンボルの本体、のどちらかだけ。検索は位置を返す、テキストの塊を返さない。**
- **コード知能インターフェース**: harness はフル索引器を内蔵しない（フル版の Python indexer は太い）。コード知能クエリ（`find-symbol` / `refs` / `callers` / `implementers` / `show-symbol` / `outline` / `deps` / `rdeps` / `closure` / `impacted-by` / `tested-by`）は harness の一級コマンドではなく、**設定されたコード知能バックエンドへの*素通し***（LSP サーバへの小さいブリッジ or Serena/MCP 経由、または事前構築 SCIP/LSIF 索引、または tree-sitter）── harness 表面では `harness query <backend-subcommand> [args]` の 1 エントリで表現する（or バックエンドが MCP/サブプロセスとして直接 expose し harness は転送するだけ）。一覧はバックエンド依存。ナレッジグラフは**キャッシュされた artifact（ファイル）であってコードではない**ので、その artifact の管理（`harness reindex` で外部索引器を叩いて再生成、`harness ckg-stale` で git HEAD に対する陳腐化チェック）は harness 側のコマンド。
- **限界**: LSP/semantic は言語依存で不完全（マクロ・コード生成・動的ディスパッチ・リフレクション・ビルド時設定・多言語 repo）。静的グラフはよく拾うが動的エッジは漏れる ── blast radius の漏れと同じ穴、だから「フルスイート遅い gate」が安全網。事前グラフは陳腐化→再索引コスト（巨大 repo で時間かかるが、それは*マシン*が払うコストで*context*は払わない）。要約・パッケージカードはドリフト→再生成トリガが要る。
- 対象コードベースが domain ごとに縦割りされていて全ファイルが小さいほど blast radius・CKG・traceability・並列化が効く ── 理想構造は `docs/target-codebase-structure.md`（助言的、harness はこの構造を要求しない）。

### 9.1 トレースマップ＝検索インデックス（"code as memory"）

- **トレースマップ（`spec.toml` の `requirement.files` ／ `requirement.tests` ── コードと設計書の双方向の紐づき）は、乖離検出（`traceability_closed` gate, §6）のためだけでなく、検索インデックスとしても使う**。F-NNN をやるとき、harness は worker の context バンドルに「その F-NNN の requirement ＋ AC ＋ 不変条件」＋「`requirement.files` のファイル（＝宣言された blast radius、依存はアウトライン・触るシンボルは本体、§9 のバジェット）」＋「`requirement.tests` のテスト」だけを入れる ── grep も探索も無い、over-fetch ゼロ。「何をロードするか」がデータ（トレースマップ）から決まる。
- **フロー**: research/scope フェーズで **CKG（`closure` / `impacted-by`）を使って影響ファイルを*発見*** → それを `spec.toml` の `requirement.files` に書く（§7・`docs/ckg.md` §7）→ 以降はトレースマップがインデックスになる（implement ノードの worker は traced files を手渡される、CKG クエリ不要）。**CKG＝「初回どう見つけるか」、トレースマップ＝「見つけたものを覚える」記録**。トレースマップ＝検索のための "code as memory"（一度確定した blast radius を spec が記憶し、次回以降の context 構築がそれを引く）。
- **双方向性**: `requirement → files` で「要件のためにどのファイルをロードするか」が引ける。逆に `file → requirement(s)` で「このファイルを見てるとき、それが奉仕する意図（＝何を壊しちゃダメか）」が引ける ── review / characterize ノードで効く（あるファイルを変えるとき、それに紐づく F-NNN の AC・不変条件を context に呼べる）。
- **紐づきはコード側にも持てる（案）**: 新規 / 変更ファイルにヘッダマーカー（`// implements: F-007` 等 ── harness がファイル作成時に付けられる）、レガシーは `spec.toml` の glob（`domains/billing/**` → F-billing 系）── これは「`--tag new` / `--tag legacy` で blast radius のスコープを変える」のと同じ流儀（`docs/operations.md` 参照）。`traceability_closed` が両方向（spec→file、file→spec）を検証する。（コード側マーカーを必須にするかは要検討 ── 10M 行で 50k ファイル全部にヘッダを付けるのは負担、glob のほうが現実的。）
- **まとめ ── 「load only what's needed」は 3 層**:
  - **意図（トレースマップ）** ── どの F-NNN がどのファイル / テストに対応するか。`spec.toml` の `requirement.files` / `requirement.tests`。「何をロードするか」を決める。
  - **構造（CKG・アウトライン）** ── 初回どう見つけるか。`closure` / `impacted-by` / `outline`。発見の道具（`docs/ckg.md`）。
  - **履歴（イベントログ・spec）** ── 抱えずクエリする。`harness status` / `harness spec` / `harness replay` で取り戻せる（§4・§9 ⑦）。

  この 3 つを「context に常駐させず、必要な瞬間に必要な分だけ引く」のが §9 全体の主張。コード知能 IF は素通し（§9 末尾「コード知能インターフェース」項、`docs/ckg.md` 参照）のまま ── harness が太らない。

## 10. エージェント topology と context 構築

**host capabilities と Phase 0↔1（要約 ── 詳細は `docs/host-capabilities.md`）**:

harness は常に sequencer ＋ gater ＋ 状態機械であり、その役割は変わらない。一方「plan（read-only research を強制した計画立案）」「security review」「code review」「人間への構造化質問」「sub-worker spawn」「編集境界の強制」という*能力*は**ホストが提供する**。Phase 0 のホストは Claude Code であり、これらは組込みで存在する（plan モード／`/security-review`／`/review`／AskUserQuestion／Agent ツール／hook）。Phase 1 のホストは harness のランタイム自身であり、等価物を harness が提供する（plan ノードの skill／`skills/security-review.md`・`skills/code-review.md`／質問キュー＋`harness ask`/`harness answer`／生 API sub-worker／tool-call インターセプタ）。

要点 ── **harness はホストの組込みを再実装せず参照する**。Phase 0 では各 skill が「ホストに `/security-review` があるならそれを invoke せよ、無ければ以下の手順」と書き、手順本体は `skills/` に移植する（thin harness, fat skills ── §2）。`[meta].host`（`"claude-code" | "runtime"`、§5・`docs/schemas.md` §2.1）でどちらのモードかを宣言する。これは「harness は hook システムを持たない」（§16・§10 のトレードオフ）と矛盾しない ── harness 自身は持たないが、ホストが Claude Code のときはそのホストの hook を活用する（Phase 0 のボーナス）。Phase 1 で hook 隔離を失う分は tool-call インターセプタが埋める（§16.2）。

- **CLI とランタイムは段階で積む（§15）**: Phase 0 は「Claude Code が叩く CLI」── このときの圧縮の上限は「Claude Code の ambient（CLAUDE.md・skill manifest・MCP ツール一覧・hooks・rules）」で頭打ち（数万トークンの前置き）。圧縮目標を真面目に取る Phase 1 では、**harness が*ランタイムそのもの*になって worker を生 API（生 Anthropic API 直叩き）で spawn し、worker の context を harness が組み立てる**（CLI コアの厳密な superset）。Rust なら公式 Agent SDK は無いが、生 API 直叩きで「context を自分で決める」利益はそのまま得られる（SDK の便利機能＝prompt caching ヘルパ・tool-use ループヘルパは自前実装）。本節以降の層モデルは Phase 1 以降の姿。
- **層モデル**:
  - **L0: harness プロセス（Rust）** ── イベントログ、gate、`workflow.toml` / `spec.toml`、ループ、worker spawn を所有。
  - **L1 の LLM オーケストレーターは*無くす*** ── ノード間の遷移は `workflow.toml` が決める（決定論的）。リトライ/エスカレ方針も `workflow.toml` の `on_reject` に書く。想定外は人間にエスカレ（§13 のエスカレ機構 ── 質問キューに `kind=escalation` のエントリを積み、`no_pending_required_questions` gate がノードをブロックする）。
  - **L2: worker（ノードごとに spawn、生 API、fresh context）** ── 渡される context = {ノードの skill、spec スライス、blast radius のファイル（依存はアウトライン、触るシンボルは本体）、そのノードに必要なツールだけ、コンパクト status}。CLAUDE.md ゼロ、ambient ゼロ。作業→「終わった」と言ったら harness が出口 gate を実行→pass: イベント commit して次ノードの worker を spawn / fail: gate の失敗を context に足してもう一度 worker を spawn（or 専用 fix worker）。worker の蓄積した思考はノード間で破棄（必要な決定はノード完了 artifact かイベントログに蒸留）＝context が改修全体で累積しない、ノードごと clean start。
  - **L3: sub-worker（ノード内）** ── 作業がでかければ L2 が並列 sub-worker を spawn、distilled な返値だけ保持。ファイルベース中継。
- **名前を付けた工夫**:
  1. ノードごと fresh context（セッション/タスクごとでなく）── 10M ステップ改修でも context が累積しない、永続するのはイベントログだけ
  2. 層間ファイル中継 ── 親はパス＋1 行サマリ、子は中身を返さない
  3. 生 API spawn で ambient bloat を殺す ── worker は CLAUDE.md/skill manifest/MCP ツール一覧/hook 設定を読まない、harness が `workflow` + `spec` + semantic クエリ結果から context を構築して渡す
  4. 決定論的オーケストレーション ── 頂点に LLM 無し、harness コードが `workflow.toml` を辿る、LLM 判断はノード内だけ
  5. ノードごとツールスコープ ── `workflow.toml` が各ノードの worker に渡すツールを宣言（research: read+semantic クエリ・edit 無し / implement: read+edit+run-command）、ツールが少ない＝ツールスキーマの context が小さい＋判断点が減る＋誤操作できない
  6. harness が worker の context を組み立てる（反転）── worker に「context を小さく保て」と責任を負わせない、harness が最小バンドルを事前に組んで初期メッセージとして渡す、worker は必要なら掘れるがデフォルトはコードがキュレート
  7. prompt cache のプレフィックス設計 ── ノード skill＋spec スライスを cache prefix にする（Anthropic の 5 分 TTL）、worker を何度 spawn しても skill 部分はキャッシュヒット
- **トレードオフ**:
  - hook 隔離を失う→「危険な bash を block」等は harness のループ内 tool-call インターセプタとして再実装が要る（gate は既に in-process なので enforcement 本体は OK だが、ガード類は要移植）。**インターセプタの責務**: 編集が blast-radius 内・コマンドが cmd-allowlist 内・cwd=worktree・no-network（`network=true` のノードを除く）を強制する ── Claude Code の hook 隔離を runtime 化で失う分をここで埋める（§16.2・`docs/operations.md` §2）
  - ノード間の直感を失う→spec スライスの質とイベントログが運ぶ決定で補う、ある程度の損失は不可避＝bounded context の代償
  - 決定論オーケストレーション→リトライ/エスカレ方針を `workflow.toml` に事前に書く必要、LLM が即興で決めるより柔軟性は落ちるが予測可能
  - Rust+生 API→tool-use ループ・prompt caching・ストリーミングを自前実装、難しくはないがコード量
  - 人間 touchpoint 機構が要る（§13）

## 11. 並列実行とマルチハーネス

### 11.1 ワークフロー内の並列（1 つの run の中）

- 既にある並列性: **L3 sub-worker（ノード内、§10 参照）** ── ノード内で作業がでかければ L2 が並列 sub-worker を spawn し、distilled な返値だけ保持する。これは「1 ノードを速く終える」並列であって、ワークフロー構造の並列ではない。
- ワークフローレベルの並列には **fork / join をモデルに足す**:
  - `fork` ノード型（N 本の並列ブランチを spawn）と `join` ノード型（全ブランチ完了を待つ）。あるいは `workflow.toml` を真の DAG として扱い、依存が満たされたノードを harness が同時実行する（fork/join はその糖衣にすぎない）。
  - **decomposition が並列を安全かつ有用にする**: plan ノードが F-007 を blast radius の互いに素な F-007.1（ファイル A,B）/ F-007.2（ファイル C,D）に割る → それらの implement ノードは並列に走れる。分解の規律が無いと並列化する単位が無い。
  - **安全条件 = blast radius が互いに素であること**。重なるノードを並列にすると衝突（マージコンフリクト / レース）。thin な選択は「重なるなら並列化を拒否」（保守的・決定論的）。判定は新 gate プリミティブ `blast_radius_disjoint(node_a, node_b)`、または harness が並列化に入る前にこのチェックを実行する。
  - **並列下のイベントログ**: 1 個の jsonl に並行書き込みするとロックが要る。クリーンなのは各並列ブランチが自分の sub-log（`state/<run-id>.<branch>.jsonl`）を持ち、join ノードがそれらをマージする方式。
  - **並列下の gate**: 各並列ノードは独立に出口 gate を評価する。**join ノードには必須の gate がある: 「全ブランチが done に到達」＋「マージ結果に対して結合/フルスイートを再実行」── 個別に green なブランチが互いを壊しうるから（integration 問題 ── §14 の「並列ブランチは個別に green でも互いを壊しうる（integration 問題）」参照）**。
  - **並列下の context**: 各ブランチの worker は自分の blast radius スライスだけを渡される ── 並列でも各 worker の context は小さいまま（§9 のバジェットがブランチごとに成立する）。
- まとめ: 「並列で速く直せる」を真面目に取るなら、**fork/join ノード型 ＋ `blast_radius_disjoint` チェック ＋ ブランチごと sub-log ＋ join での再検証ノード**を要する。decomposition の規律（互いに素な blast radius を持つ独立 F-NNN）が並列の前提。

### 11.2 複数ハーネスの同時実行

- **(a) 独立タスクを別々の harness run で同時に**（変更 X 用と変更 Y 用を同時起動）: 各 run は自分の `state/<run-id>.jsonl`・自分の `spec.toml` / `workflow.toml` を持ち、run-id で隔離される。リスクと対処:
  - **ファイル隔離 = 各 run に作業ディレクトリを 1 つ（典型的には git worktree）**。`--worktree <path>` フラグは「以降この run の `cmd_exit_0` と編集の作業ディレクトリをここにする」だけを意味し、**worktree 自体の作成/破棄は harness が所有しない** ── ユーザー or 外部ツール（`git worktree`、`C:\ツール\git-worktree-runner` 等）の責任。複数 run = 複数作業ディレクトリ = ファイル衝突なし。harness のコードは「どのディレクトリでコマンドを走らせ編集を制限するか」を知るだけ。
  - **テスト内の共有外部状態（DB・ポート・ネットワーク）は worktree では解決しない** → テストが hermetic（per-run DB・動的ポート）であるか、共有リソースを使う run は直列化するしかない。harness は per-run scratch ディレクトリ/env を提供できるが完全には解決できない ──「並列 run にはテストの hermetic 性が要る」と文書化する。
  - **コードナレッジグラフ/索引は共有・read-mostly** → 並行読みは OK。`reindex` は atomic swap（temp に書いて rename）で並行読みと干渉しないようにする。
  - **API レート制限・コスト**: N 並列 = N 倍の Anthropic API 呼び出し ── 実用上の上限であって設計の欠陥ではない。
- **(b) 1 つの巨大変更を複数 harness で協調処理**（11.1 を粗い粒度で）: 巨大変更を K スライスに並列化したい場合は「コーディネーター用 `workflow.toml`」を書く ── そのノードが `cmd_exit_0` で子 run（`harness start ...`、各々が自分の作業ディレクトリを持つ）を起動し、最後のノードが merge & validate（`cmd_exit_0 "git merge ... && <フルスイート>"` ＋ `traceability_closed`）。これは harness の新コードを必要としない、`workflow.toml` の書き方のパターン。前提は (a) と同じ（互いに素な blast radius、hermetic テスト or 直列化、作業ディレクトリ隔離）。各ブランチ／子 run の作業ディレクトリの用意は外部の仕事。

### 11.3 これでまだ thin か（トレードオフ）

- 足すもののほとんどは**データ＋プリミティブ＋シェルアウト**: 並行可能性は `workflow.toml`（データ）の DAG/fork-join に書く / blast-radius 互いに素チェックは gate プリミティブ 1 個 / 作業ディレクトリ（worktree）の用意は外部ツールの仕事 ── harness は `--worktree <path>` で「どこで走らせるか」を受け取るだけ / merge-validate ノードは `cmd_exit_0 "git merge ... && <フルスイート>"` の gate を持つただのノード。
- 本当に**新規の harness コード**になるのは: 一個ずつでなく DAG ノードを並行実行するスケジューラ。実体のある追加だが bounded。worktree のライフサイクル（作成/破棄/隔離）は harness が所有しない（§11.2）。
- **確定**: fork/join はコアの `workflow.toml` モデルに入れる。ただし**並行実行は Phase 2**（並列スケジューラ、ランタイム前提、§15）── 各ブランチ／run の作業ディレクトリの用意は外部。**それまでの Phase 0/1 では `fork`/`join` ノードはブランチを逐次に実行する（degrade）** ── モデル上は DAG だが実行は逐次。並列性は当面 L3 sub-worker と「別々に起動した独立 run」（§11.2）で得る。

## 12. コマンド / CLI 表面

harness が所有するコマンドは 3 群:

- **状態機械系**: `start` / `status` / `advance`（別名 `request-transition`）/ `back` / `record-artifact` / `report-evidence` / `ask` / `questions` / `answer` / `reset` / `abandon` / `stuck`
- **問い合わせ系（コンパクト）**: `skill` / `spec` / `gates`
- **運用系**: `init` / `doctor` / `validate` / `inspect` / `replay` / `stats` ＋ `reindex` / `ckg-stale`（CKG キャッシュ artifact の管理）

`harness init` は onboarding スキャフォールド（既存 repo に `workflow.toml` / `spec.toml` のひな型・`skills/` を置き、内部で `harness validate` を実行し、スモークチェックする ── 詳細は `docs/onboarding.md`）。`harness doctor` はスモークチェックを再実行し、config / skill / ツール設定のドリフトを flag する。

コード知能クエリ（§9）は harness の一級コマンドではなく、設定されたコード知能バックエンドへの**素通し** ── harness 表面では `harness query <backend-subcommand> [args]` の 1 エントリで表現する（典型の subcommand: `find-symbol` / `refs` / `callers` / `implementers` / `show-symbol` / `outline` / `deps` / `rdeps` / `closure` / `impacted-by` / `tested-by` ── 一覧はバックエンド依存）。`reindex` / `ckg-stale` は CKG キャッシュ artifact を扱うので harness 側のコマンド。

**各コマンドの引数・効果・状態を変えるか・委譲の有無は `docs/schemas.md` §4 が唯一の正典**（CKG コマンドの完全な説明は `docs/ckg.md` §6）。`gh pr create` のような「成功した run の成果物から PR を作る」は workflow.toml の最終ノードに `cmd_exit_0 "gh pr create ..."` を 1 行書けば済む ── harness 機能ではない（§16.5）。

## 13. 人間の touchpoint

完全自律ループだと人間が触るのは 3 種だけ: **(a) 凍結 spec の承認** / **(b) 壁打ち中の `??`・曖昧さの解消** / **(c) escalation（想定外）のエスカレ**（旧 `human_decision`。§4 で `human_answer`(kind=escalation) に統合）。

### 13.1 対話モデル — 3 種すべて AskUserQuestion 方式

3 種すべて、対話モデルは **AskUserQuestion 方式**: 短いヘッダ ＋ 2〜4 個のラベル付き選択肢（各々に説明）＋ 常時利用可の自由記述 other。理由 ── 決定的（回答が状態遷移、または `spec.toml` への値書き込みに、きれいに対応する）／曖昧さが小さい／context を膨らませる open-ended chat を避ける。これは harness の決定論思想と整合する（open-ended な雑談を排し、回答を構造化レコードに落とす）。

### 13.2 surface の transport — 質問キュー

- harness は保留中の質問を**質問キュー**（`state/<run-id>.questions.jsonl`、append-only）に書く。各エントリ:

  ```json
  {"id": "...", "kind": "spec_approval|clarification|escalation",
   "header": "...", "question": "...",
   "options": [{"label": "...", "description": "..."}],
   "required": true, "context_ref": "F-003"}
  ```

- 人間（または薄い UI / CLI）がキューを watch し、`harness answer <question-id> <option-index | "自由記述">` で回答 → harness が `human_answer` イベントを append する。`kind=clarification` の場合、harness は回答を `spec.toml` の該当要件（`context_ref` で示される F-NNN の `[[open_question]].answer`、ないし `??` 箇所）に書き込み、その `??` をクリアする。
- harness は **`no_pending_required_questions` gate**（質問キューに `required: true` で未回答のエントリが無い）で、該当ノードの進行をブロックする ── このノードが進めるのは「積んだ必須質問が全部回答済み」になってから。
- 質問が積まれたら `PushNotification` / hook で人間に通知してもよい（任意・実装裁量）。

### 13.3 各種別の出し方

- **(a) spec 承認**: spec ノード出口で worker が
  `{kind:"spec_approval", header:"spec承認", question:"この spec はあなたが欲しい変更か?（要約: ...）", options:[{label:"承認", description:"凍結して implement へ"},{label:"修正が要る", description:"壁打ちに戻す（理由を other で）"}], required:true}`
  をキューに積む。「承認」が返ったら worker は `harness report-evidence human_approval '{"verdict":"approved"}'` → `json_has human_approval verdict eq approved` gate が pass する。
- **(b) clarification（`??`）**: research / spec ノードで worker が曖昧さに当たったら `harness ask "<質問>" --option "<A>" --option "<B>" [...]` でキューに積む（`kind=clarification`。自由記述の `??` ブロックを `spec.toml` 本文に書いてもよいが、その場合は options なしのエントリになる）。人間が回答 → `spec.toml` に書かれる。`open_questions_zero` gate は全部回答され `??` が消えるまで fail のまま。
- **(c) escalation**: `on_reject` が `goto="__human__"` に当たったら harness（または worker）が
  `{kind:"escalation", header:"エスカレ", question:"ノード X が K 回 reject。どうする?", options:[{label:"plan に戻す"},{label:"このノードの gate を見直す"},{label:"中断"}], required:true}`
  をキューに積む。回答 → 対応するイベント / 遷移（plan へ `back`、gate 修正、run 中断）。

これで「人間 touchpoint の surface 機構」は**機構レベルで確定**: 質問キュー（`state/<run-id>.questions.jsonl`）＋ `harness ask / questions / answer` ＋ `no_pending_required_questions` gate ＋ 任意の通知。**残るオープン論点**（§17）: キューの上に載せる UI（CLI のみか、薄い TUI / web か）、実際のチャット面（Claude Code の AskUserQuestion / Slack 等）との統合をやるか。詳細な context 構築との関係は `docs/worker-context.md`、スキーマは `docs/schemas.md`。

## 14. 正直な限界（まとめて再掲）

> 失敗モードの系統的カタログは `docs/failure-modes.md` を参照。本節は要点。

**harness が*保証する*もの**:
- (a) 壊れた中間状態は下流に伝播しない（出口 gate を満たさないノードは進めない、§4・§6）
- (b) done に到達したならその状態は宣言された全 gate を満たす（`cmd_exit_0` は harness 自身が実行、嘘がつけない、§7・§8）
- (c) 人間のレビュー負荷は O(spec)（diff でなく spec を見る、§2・§13）

**harness が*保証しない*もの**:
- (d) **spec が真の意図を捉えているか** ── 人間の入力に依存する。spec 承認（§13）がその一点を担うが、人間を正しくはできない。
- (e) **テストが完全か** ── coverage gate で下限、AC↔test 必須で意図の各項目にテストがあるところまで、mutation testing（任意 gate）で「意味のあるテスト」の下限 ── どれも証明ではない。動的依存・テストの無い経路は ship できる（フルスイート遅い gate が安全網だがそれも漏れる）。
- (f) **ノード内のエージェントのアプローチが最適だったか** ── plan-approval gate（§13）で人間は plan も見るが、中身の良さは L5 で gate できない（L5 gate 禁止、§7）。悪いアプローチは `harness back` で self-correct する。
- シークレット漏洩は**残存リスク** ── context に入れたものは API に渡る、redaction は best-effort、`gitleaks` 系 gate は「ソースに書いた」分だけ捕まえる（§16.2）。

以下は上記の各項目の細目（再掲）:

- spec が*真の*意図を捉えているかは保証しない（人間の入力。spec 承認がその一点を担う）
- テストの*完全性*は証明できない（coverage gate で下限は引ける、AC↔test 必須で「意図の各項目に対応するテストがある」までは保証）
- blast radius を*完全に*特定できる保証はない（隠れた依存・動的呼び出し・設定経由）→「フルスイート遅い gate」が安全網
- characterization test は「今の挙動」を固定するだけで「今の挙動が正しい」とは言ってない（バグごと固定することがある。改修の意図に「そのバグも直す」が無い限り現状維持が安全）
- LSP/semantic は言語依存で不完全（§9 の限界）
- 事前グラフ・パッケージカードは陳腐化する（再生成トリガが要る）
- hook 隔離を失う（§10 のトレードオフ）
- ノード間の直感を失う（bounded context の代償）
- エージェントはノード内で試行錯誤しうる（が安く・見えず・伝播しない）
- 並列ブランチは個別に green でも互いを壊しうる（integration 問題）→ join ノードでのマージ結果再検証が必須（§11.1）
- 並列 run の共有外部リソース（DB・ポート・ネットワーク）は harness では隔離できない → hermetic テスト前提、さもなくば直列化（§11.2）。（設計判断: gate では強制しない ── 決定論的に hermetic 性を検出できないため。代わりに per-run scratch ディレクトリ/env を提供し、「並列 run にはテストの hermetic 性が要る」と文書化する）
- worker の context に入れたシークレットは API に渡る ── `secrets_glob` の除外と出力 redaction は提供するが、完全な保証ではない（§16.2）
- テストが「意味のある」テストか（assert がトリビアルでないか）は決定論的に検出できない ── mutation testing を任意 gate にすれば下限だけは引ける（§17・`docs/operations.md` §6）
- flaky / 不十分なテストスイートは harness が直せない（リトライ・隔離はできるが、スイートの質そのものは外の問題、§17・`docs/operations.md` §6）
- サンドボックス（FS 権限・コンテナ・no-network）の効き方は OS / 実行環境に依存する（§16.2）

## 15. 現状と次のステップ（段階制で確定）

- **現状**: `C:\ツール\thin-workflow-harness\src\*.rs` と `skills/*.md`（現 `01-research.md`〜`05-review.md`）は v0 prototype（5 フェーズをコードにハードコード、gate を名前で match）。**この設計の方向に作り直す**（`phases.rs` を捨てて `workflow.toml` + プリミティブ gate に、`gates.rs` をプリミティブのみに、`skills/` を連番 8 種 `01-research.md`〜`08-join.md` に置換 ── v0 skills は番号が一部ズレるので捨てて作り直す、`docs/skill-templates.md` 冒頭）。

- **onboarding**: 既存 repo に harness を乗せる手順は `harness init`（スキャフォールド＋`harness validate`＋スモークチェック）と `harness doctor`（スモークチェック再実行・ドリフト flag）── §12・`docs/schemas.md` §4・`docs/onboarding.md`。

- **ブートストラップ問題**: harness を作る作業自体に harness を使いたいが、まだ harness が無い ── Phase 0 のコアは手で（ホスト＝Claude Code の組込み plan モード・`/review` 等で）作り、それができたら以降は dogfooding（harness 自身の改修を harness の workflow.toml で回す）。

- **worked example / failure catalog**: 10M 行級改修を end-to-end でトレースした例は `docs/example-walkthrough.md`、失敗モードの系統的カタログは `docs/failure-modes.md`。

- **「CLI 版を中間に作るか」は解決した**: CLI 版 vs ランタイムは二者択一ではない。**CLI コアを先に作り、ランタイムはその厳密な superset を後で足す。同じ土台。CLI コアは捨てプロトタイプではなく以降の全フェーズの土台。** 以下の段階制で進める。

> 実装言語・クレート・hook の方針・ソースのディレクトリ構成などの実装レベルの決め事は `docs/implementation.md`。

### Phase 0 — CLI コア

append-only イベントログ＋`derive_state`（§4）、プリミティブ gate 評価器（§7）、`workflow.toml` / `spec.toml` の config ローダー（§5・§6・`docs/schemas.md`）、`harness` コマンド表面（状態機械系・問い合わせ系・運用系 ＋ コード知能クエリの素通し（`harness query ...`）── 一覧は §12、正典は `docs/schemas.md` §4）。

- ノードは**厳密に逐次実行**: `workflow.toml` に `fork`/`join` があっても Phase 0 はブランチを逐次に実行する（並行実行は Phase 2、それまで逐次 degrade、§11.3）。
- エージェントは Claude Code が `harness` コマンドを叩く形（生 API spawn はまだ）。
- これで状態機械・gate・`workflow.toml`/`spec.toml` モデルを実人間込みで end-to-end 検証する。**ここで作るコアは throwaway ではない** ── 以降のフェーズはこの上に積む。
- 圧縮効果はこの段階では限定的（状態を抱えずクエリ／skill on-demand／grep より semantic／簡潔出力 ── per-node fresh context はまだ Phase 1）。
- **Phase 0 が実際に出すもの（正直に）**: Phase 0 では圧縮効果は限定的（per-node fresh context は Phase 1）、並列なし（fork/join は逐次 degrade）、CKG なし（コード知能クエリは LSP オンデマンドフォールバック or 未提供）。残るのは「config 駆動の状態機械＋プリミティブ gate＋`traceability_closed`＋`cmd_exit_0` によるテスト配線、Claude Code が `harness` コマンドを叩く形」── 実質、現 `src/*.rs`（v0 prototype、5 フェーズハードコード）を config 駆動にした程度。4 大目的のうち Phase 0 で立つのは「設計書とコードの同期」と「テスト層の自動担保」だけで、「圧倒的に少ない context」「10M ステップ改修を一発で」は Phase 1（ランタイム）に丸ごと依存する。したがって Phase 0 は「製品の節目」ではなく「Phase 1 の最初のコミット（共有される core lib の検証）」と位置づけるのが正しい ── core lib（小さい）を作ったら、Phase 0 を完成させてから Phase 1 ではなく、walking skeleton で Phase 1 まで薄く貫く。

### Phase 1 — ランタイム層

生 Anthropic API で worker を spawn、context バンドル構築（`docs/worker-context.md` 準拠）、tool-call インターセプタ（blast radius 内に edit を制限）、prompt caching、worker ライフサイクル（§10）。これで per-node fresh context の圧縮が効く（§9・§10 の名前付き工夫）。

### Phase 1.5 — CKG バックエンド（Phase 1 と並行可）

`docs/ckg.md` 準拠の索引器連携を semantic コマンド（`find-symbol` / `refs` / `closure` / `impacted-by` / `tested-by` 等）に配線する。それまでこれらは LSP オンデマンドにフォールバック（or 未提供）。

### Phase 2 — 並列スケジューラ

`fork` ブランチの並行実行、`join`、ブランチごと worktree（＋独立 run 用の `--worktree`、§11.2）。ランタイム（Phase 1）が前提。それまで `fork`/`join` は逐次に degrade。

### skillify（`docs/skillify.md` 準拠）

opt-in の retrospective ノード＋playbook（再発する変更パターンを skill 化して再利用＝複利、§2・§5）。Phase 0 以降いつでも追加可（ほぼデータ＋1 ノード）。

### 運用面（§16）の入れどころ

resilience / 予算（§16.1）・セキュリティ（§16.2）・可観測性（§16.3）・config 検証（§16.4）・deliverable ライフサイクル（§16.5）は Phase 0〜1 で並行して入れる。`harness validate`・`abandon`・スキーマ版のイベントログ記録は Phase 0（ランタイム不要）。budget・`node_aborted`・tool-call インターセプタ（cmd-allowlist / no-network）・transcripts・ノードごと `model` / `cmd_allowlist` / `network` の効力・`harness stats` のトークン計測は主に Phase 1（ランタイムの振る舞いが多いため）。

## 16. 運用上の考慮事項

> 本節は要約。詳細・運用手順・CLI の細目は `docs/operations.md`（§1〜§7）に分けてある。runtime 化（§10・§15 Phase 1）で Claude Code の hook 隔離・ambient ガードを失う分を、ここで列挙する仕組みが埋める。

### 16.1 失敗・中断・タイムアウト・予算

- **クラッシュ復旧**: イベントログ（§4）が SSOT なので、harness が死んでも最後にコミットされたイベントから再開できる。中途のノード（`advance` も `node_aborted` も書かれていないノード）は fresh worker で再 spawn ── worker の途中状態は破棄（`--worktree` モードならその worktree の未コミット編集ごと捨てて作り直す）。
- **gate タイムアウト**: `cmd_exit_0` の gate コマンドには runtime がタイムアウトを適用する（既定値、args の `timeout_seconds` で上書き）。タイムアウトした gate は fail 扱い。
- **API リトライ**: worker spawn 中の Anthropic API エラー（429 / 5xx）は指数バックオフで自動リトライ。リトライ尽きたら `node_aborted{reason: api_error}`。
- **ノードごと予算**: ノードに `budget`（`{max_tool_calls, max_tokens, max_wall_seconds}`、§5）を付けられる。超過したら `node_aborted` → そのノードの `on_reject` に従う（リトライ or `__human__` エスカレ）。run 全体には `[meta].run_cost_budget`（超過で人間にエスカレ）。
- **worker の詰まり自己申告**: worker が「これ以上進めない」と判断したら `harness stuck "<理由>"` で正直に申告 → `node_aborted{reason: stuck}` ＋ エスカレ（質問キューに `kind=escalation`、§13）。無理に `request-transition` を空打ちさせない。
- **メトリクス記録（サイドカー）**: ノード完了時のメトリクス（`cost`, `tokens`, `tool_calls`, `wall_seconds`）は `advance` イベントには載せず、append-only サイドカー `state/<run-id>.metrics.jsonl`（各行 1 ノード分の `{node, cost, tokens, tool_calls, wall_seconds, ts}`）に書く ── イベントログを軽く保つための分離。`harness stats <run-id>` はこのサイドカーを読む（§16.3）。run 全体のコスト累計は `harness status` / `harness stats` で表示。
- **ノードごとモデル選択**: ノードに `model`（§5）。重いノードは大きいモデル、軽いノードは小さいモデル。

→ 詳細は `docs/operations.md` §1。

### 16.2 セキュリティ・信頼境界

worker は LLM が提案したコマンドを実行する ── 脅威は (a) 破壊的操作（`rm -rf` 等）、(b) シークレット流出（context に入れた鍵が API に渡る／コマンド出力でログに残る）、(c) repo コンテンツによる prompt injection（読んだコードに「これまでの指示を無視せよ」が埋まっている）、(d) 悪意ある skill。防御:

- **コマンド allowlist**: `run-command` ツールが受け付けるのは `cmd_allowlist`（ノード単位、§5）にマッチするパターンだけ。`cmd_exit_0` の gate コマンドは `workflow.toml` に事前宣言済みなので暗黙的に許可。
- **サンドボックス**: `--worktree` モードの worktree ＋ FS 権限（編集は blast radius 内に制限）＋ 任意でコンテナ。runtime のループ内 **tool-call インターセプタ**が、編集が blast-radius 内・コマンドが cmd-allowlist 内・cwd=worktree を強制する（Claude Code の hook 隔離を runtime 化で失う分をここで埋める、§10）。
- **ネットワーク**: no-network がデフォルト。ノードに `network = true`（§5）を付けたときだけ例外。
- **シークレット redaction**: `[meta].secrets_glob`（§5）にマッチするファイルは worker の context に入れない。コマンド出力・transcripts も既知パターンを redaction。**ただし worker の context に入れたものは API に渡る ── redaction は best-effort であって完全な保証ではない**（§14）。
- **監査ログ**: イベントログ＋transcripts（§16.3）が監査証跡。

→ 詳細は `docs/operations.md` §2。

### 16.3 可観測性・デバッグ

- **transcripts**: 各 worker の全会話（送ったプロンプト・受けた応答・tool-call とその結果）と、harness が組み立てて渡した context バンドルそのものを `state/<run-id>.transcripts/` に保存。
- **gate ログ / コマンドログ**: gate 評価ごとの結果（pass/fail＋note）、`cmd_exit_0` で走らせた各コマンドの cmd・exit code・出力（要約）。
- **CLI**: `harness inspect <run-id> [--node X]`（ノードの状態・gate・artifact・transcript への参照を見る）/ `harness replay <run-id>`（イベントログを頭から fold して状態遷移を再現）/ `harness stats <run-id>`（ノードごとの context トークン数・コスト・tool-call 数 ── サイドカー `state/<run-id>.metrics.jsonl`（§16.1）を読む。**`harness stats` の context トークン数が「圧倒的に少ない context」を*測る*手段**、§9）。

→ 詳細は `docs/operations.md` §3。

### 16.4 config 検証

- **`harness validate [--workflow path] [--spec path]`**: `workflow.toml` / `spec.toml` の静的検証 ── `next` / `branches` / `wait` の参照先が実在ノードか・`[meta].entry` が実在か・`next` で前方サイクルを作っていないか（**`next` 前方サイクルは error** ── 前ノードへ戻れるのは `back` / `on_reject` の `goto` 経由のみ）・`exit_gates` の gate 名と args が妥当か・`[[node]].serves` の F-ID が `spec.toml` に実在するか・`skill` のファイルが実在するか・全ノードが到達可能かつ停止するか・`mandatory_gates` の spec が妥当か。状態を変えない。
- **自動実行**: `harness start` 時に `validate` を自動で走らせる ── 壊れた config はノード途中でなく start で落とす（fail-fast）。

→ 詳細は `docs/operations.md` §4。

### 16.5 deliverable のライフサイクルと spec amendment

- **成功**: 成果物は diff／ブランチそのもの（`--worktree <path>` で指定した作業ディレクトリの diff）。成功した run の成果物（diff/ブランチ）から PR を作るのは workflow.toml の最終ノードに `cmd_exit_0 "gh pr create ..."` を 1 行書けば済む ── harness 機能ではない。
- **失敗 / 中断**: `harness abandon <run-id>` は `abandon` イベント（run を放棄状態にする、理由を payload に）を書く ── イベントログが SSOT なので run 状態は必ずイベント経由でマークされる。ファイルシステム上の worktree の後始末（`--worktree` モードなら `git worktree remove` 等、そうでなければ `git reset`）はイベントとは別の外部作業。worktree の作成/破棄は harness が所有しない（§11.2）。
- **spec amendment**: 途中で要件が変わったら spec ノードに `back` → `[meta].status` を `draft` に戻す → 壁打ち再開 → 再承認。これで無効化された implement 成果物（変わった F-NNN に紐づいていた artifact）を `traceability_closed` が orphan として検出 → その implement ノードも `back` させる。**amendment は高くつく＝意図的な摩擦**（軽々しく要件を変えさせない）。

→ 詳細は `docs/operations.md` §5。

## 17. オープンな論点（未定、要議論）

解決済みの論点（参照先）:

- 「CLI 版を中間に作るか」→ §15 段階制（CLI コア→ランタイム superset、同じ土台）
- 「CKG インクリメンタル更新の粒度」→ `docs/ckg.md`（ファイル単位＋逆依存閉包＋フルフォールバック。hot symbol の劣化は既知の限界）
- 「fork/join スケジューラをコアに入れるか」→ コアに入れる（並行実行は Phase 2、それまで逐次 degrade、§11.3）
- 「hermetic テストを強制する gate を置くか」→ 置かない（決定論的に hermetic 性を検出できない。並列 run の要件として文書化＋per-run scratch env、§11.2・§14）
- 「`characterize_before_change` をプリミティブにするか `cmd_exit_0` か」→ `characterize` ノードの出口 gate を `cmd_exit_0`（カバレッジチェック）にする（新プリミティブ不要、§7）
- 「skillify の仕組み化」→ `docs/skillify.md`
- 「`workflow.toml` の append 細則」→ §5 に細則を記載
- 「`harness validate` のサイクル方針」→ `next` 前方サイクルは error（§16.4・`docs/operations.md` §4）
- 「`abandon` がイベントを書くか」→ `abandon` イベントを書く（§4・§16.5・`docs/schemas.md` §5）
- 「ノード完了メトリクスをイベントに載せるかサイドカーか」→ サイドカー `state/<run-id>.metrics.jsonl`（§16.1・§16.3・`docs/operations.md` §1）
- 「host 分岐の skill 表現（テンプレート構文か否か）」→ skill は常に手順を持ち host 組込みがあれば優先（§10・`docs/host-capabilities.md` §3）

残る論点:

- prompt caching の粒度（現状の最小版は `docs/worker-context.md` B4。最終確定は要検討）
- 人間 touchpoint：キュー上の UI（CLI のみか、薄い TUI / web か）、外部チャット面（Claude Code の AskUserQuestion / Slack 等）との統合をやるか
- playbook の「再発」検出ヒューリスティックの調整（`docs/skillify.md`）
- tool-call インターセプタの具体（blast radius / cmd-allowlist / cwd / no-network 強制の実装方法、Phase 1。§10・§16.2）
- worktree 隔離の粒度（run ごと？ ブランチごと？、§11.2）
- 並列ブランチ sub-log のマージ方式（§11.1）
- flaky / 不十分なテストスイートへの対処（リトライ・隔離・harness は直せない、詳細は `docs/operations.md` §6）
- 長時間テストの非同期化（CI に push → poll、`cmd_exit_0` を待つだけ、詳細は `docs/operations.md` §6）
- mutation testing を任意 gate にするか（テストの「意味」の下限を測る、`cmd_exit_0` で表現、詳細は `docs/operations.md` §6）
- 「ビルドが通る」（`cmd_exit_0 "cargo check --workspace"`）を `[meta].mandatory_gates` に入れるか（詳細は `docs/operations.md` §6）
- lessons log（playbook / retrospective ログ）肥大化対策（詳細は `docs/operations.md` §6）
- harness 自身のバージョニング（スキーマ版をイベントログに記録、詳細は `docs/operations.md` §6）
- マルチ言語モノレポでの CKG マージとサブツリーごとのツール設定（`docs/ckg.md` も触れる、詳細は `docs/operations.md` §6）
