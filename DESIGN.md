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

harness が**所有するもの**（薄いまま、ほぼ未来永劫変わらない）:

1. イベントログ（append-only jsonl）+ `derive_state`（純粋 fold）
2. ループ / コマンド表面（status / request-transition / back / record-artifact / report-evidence / reset / ask / questions / answer ＋ コンパクト問い合わせ群 ＋ reindex ＋ start）
3. プリミティブ gate 評価器 ~15 個前後（汎用・意見ゼロ・config で引数化）（§7）
4. config ローダー: `workflow.toml`（ノード/エッジ/出口 gate）＋ `spec.toml`（F-NNN/AC-N/トレースマップ、optional）
5. ノードごとに worker を spawn し、その context を組み立てるランタイム（§10）

harness が**持たないもの**:

- フェーズの名前（research/plan/… は `workflow.toml` に書く）
- 「200」という数字、禁止語リスト（`workflow.toml` で `max_lines` / `no_regex` の引数として渡す）
- プロジェクト固有ロジック（それは fat code。`cmd_exit_0` で呼ぶだけ）
- LLM 判断 gate
- フル索引器（コード知能は外部に委譲、§9）
- 頂点の LLM オーケストレーター（オーケストレーションは決定論的、§10）

```
harness バイナリ（thin, 変わらない）
  ├ event log + derive_state
  ├ loop / コマンド（status / request-transition / back / record-artifact / report-evidence / reset / ask / questions / answer / 問い合わせ / reindex / start）
  ├ プリミティブ gate ~15種（汎用・意見ゼロ）
  ├ config ローダー（workflow.toml + spec.toml）
  └ worker ランタイム（ノードごとに生APIで spawn、context を harness が構築）
workflow.toml（データ、編集可能）── プロセス: ノード/エッジ/出口gate。実行中に append できる
spec.toml（データ、optional）── 設計書の検証可能部分: F-NNN / AC-N(各々に testコマンド) / トレースマップ / 不変条件 / open_questions
skills/*.md（markdown, fat）── 各ノードで何を作るか / record-artifact・report-evidence の使い方
コード知能バックエンド（外部、プラガブル）── LSP / SCIP/LSIF 索引 / tree-sitter
fat code（外部）── プロジェクトのテスト・linter・coverage・IF署名チェック ── cmd_exit_0 で呼ばれるだけ
```

並列実行（fork/join ノード、複数 run の同時実行、worktree 隔離）については §11 参照。playbook / skillify（再発パターンの再利用）は §2・§5 と `docs/skillify.md`、コードナレッジグラフ（上図「コード知能バックエンド」の実体）は `docs/ckg.md` を参照。

## 4. 状態モデル

- **append-only イベントログ（jsonl、`$HARNESS_HOME/state/<run-id>.jsonl`）が SSOT**。各行 1 JSON、共通フィールド `ts`（ISO8601 UTC）。
- イベント種別:
  - `start` `{intent}`
  - `advance` `{from, to}`
  - `advance_rejected` `{failed_gates:[{gate, reason}]}`
  - `back` `{reason}`
  - `artifact` `{name, path, tag?}`
  - `gate_evidence` `{gate, data}`
  - `reset`
  - `node_appended` `{node_def}`（plan ノードが workflow を拡張したとき）
  - `question_queued` `{question}`（worker が `harness ask` で構造化質問を質問キューに積んだとき。`question` は `{id, kind, header, question, options, required, context_ref}`、§13）
  - `human_answer` `{question_id, answer}`（人間が `harness answer` で回答したとき。`kind=escalation` の回答は従来の `human_decision` を兼ねる ── `human_decision` は `human_answer`(kind=escalation) に統合した）
  - `branch_forked` `{branch_ids}`（fork ノードが並列ブランチを開始したとき。各ブランチは自分のイベントを `state/<run-id>.<branch>.jsonl` に書く、§11.1）
  - `branch_joined` `{branch_ids, merge_result}`（join ノードが全ブランチをマージし検証したとき、§11.1）
- `derive_state(events) -> State`: 純粋 fold。同じイベント列は必ず同じ状態 ＝ 文字通り決定論的。`reset` が来たら「それ以降のイベントだけ」で再構築（ただし `run_id` / `intent` は最初の `start` から保持）。`advance` で phase_index +1、`back` で saturating -1、`artifact` / `gate_evidence` で map 更新（同名上書き）。
- **LLM は write-state できない**。`record-artifact name path` / `report-evidence gate json` で「リクエスト＋根拠」を出すだけ。harness が path の実在を確認し、json をパースし、その上でイベントを書く（自己申告を信じない）。`write-state` も `skip` もコマンドに存在しない ── これが thin harness の安全性そのもの。
- セッション / worker をまたいで状態を読み直すときは必ずイベントログから derive する。LLM の context に状態を持ち越さない。

## 5. ワークフローモデル

- `workflow.toml` がノードを定義: `id`, `skill`（`skills/` 配下のファイル名）, `exit_gates`（プリミティブ名＋引数のリスト）, `next`（次ノード id、複数候補可）, `on_reject`（N 回 reject されたら遷移する先 ── リトライ/エスカレ方針を*ここに書く*）, `tools`（このノードの worker に渡すツールのリスト ── ノードごとツールスコープ）, `artifact_tags`（このノードで登録する artifact の tag と、tag ごとに課す追加 gate）。
- ノードは**1 個でも、線形 5 個（research→plan→implement→test→review）でも、DAG でも自由**。「research/plan/…」は 1 つの `workflow.toml` の中身であって harness の概念ではない。
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

## 7. gate プリミティブ（確定リスト）

各 gate は `(state) -> (ok: bool, note: String)`（fail なら note=理由、pass なら一言）の純粋関数。`eval_gate(name, args, state)` で評価。未知の名前は `ok=false`。汎用・意見ゼロ・config で引数化。**名前付き合成 gate（"implement の出口" のような塊）は `workflow.toml` / `spec.toml` 側に書く。harness 内はプリミティブのみ。**

| 名前 | 引数 | 定義 |
|---|---|---|
| `file_exists` | `path` | path が実在ファイル |
| `file_nonempty` | `path` | path が実在ファイルかつ中身非空 |
| `max_lines` | `path`, `n` | path の行数 ≤ n |
| `lines_not_increased` | `path`, `baseline_key` | path の行数が baseline（evidence に記録された値）以下（レガシーファイル改修用：「今より増やすな」） |
| `no_regex` | `path`, `pattern` | path のテキストに pattern がマッチしない（禁止語チェック等。複数 path をグロブで指定可） |
| `cmd_exit_0` | `cmd` | シェルコマンド cmd を実行して exit code が 0（テスト・linter・coverage・IF 署名チェックなど何でも。harness 自身が実行する＝self-report を信じない） |
| `json_has` | `evidence_key`, `json_path`, `eq?` | `gate_evidence[evidence_key]` が存在し、`json_path` の値が存在（`eq` が指定されればその値と等しい） |
| `artifact_registered` | `name_or_prefix` | その名前（または `impl:` のような prefix）の artifact が ≥1 件登録され、全て実在ファイル |
| `evidence_recorded` | `key` | `gate_evidence[key]` が存在する |
| `traceability_closed` | （なし） | §6 の定義 |
| `workflow_append_only` | （なし） | §5.1 の許可/禁止リストを検証 ── 許可: 新規 `[[node]]` の追加・未到達ノードへの `next`/`branches`/`wait` 配線追加。禁止: 既存ノードの変更/削除・既存 `exit_gates` の削除/弱体化・`on_reject` の緩和・既存ノードへのツール追加・`context` の拡大・`[meta].entry` の変更。さらに新規ノードは `[meta].mandatory_gates` を満たすこと。append できるのは `can_append = true` のノードだけ |
| `count_non_decreasing` | `evidence_key`, `baseline_key` | `gate_evidence[evidence_key]` の数値が baseline 以上（`tests_count_non_decreasing` 等に使う） |
| `open_questions_zero` | （なし） | §6 の定義 |
| `blast_radius_declared` | （なし） | spec の各 F-NNN に「影響ファイル ≥1」が紐づいている |
| `no_pending_required_questions` | （なし） | 質問キュー（`state/<run-id>.questions.jsonl`）に `required: true` で未回答のエントリが無い（§13） |

注: `traceability_closed` / `workflow_append_only` / `open_questions_zero` / `blast_radius_declared` は汎用なので harness 内プリミティブにする価値あり。

**characterization について**: 「変える前に characterization test を書け」は専用プリミティブ（旧 `characterize_before_change`）にはしない。代わりに、implement の前に `characterize` ノードを置き、その出口 gate を `cmd_exit_0`（カバレッジツールを影響ファイルに対し走らせ、閾値未満なら fail）にすることで強制する。例: `{ gate = "cmd_exit_0", args = { cmd = "<カバレッジツール> --affected <blast_radius> --fail-under 80" } }`（`docs/schemas.md` §2 の `characterize` ノード例参照）。

## 8. テスト層の自動担保

- 単体 / 結合 / E2E / カバレッジ = それぞれ `cmd_exit_0 <command>` gate を該当ノードの `exit_gates` に配線するだけ。harness はテストを特別扱いしない。例: implement ノード出口に `cmd_exit_0 "cargo test --lib"`、integration ノードに `cmd_exit_0 "cargo test --test '*'"`、e2e ノードに `cmd_exit_0 "./e2e.sh"`、`cmd_exit_0 "cargo llvm-cov --fail-under-lines 95"`。
- **速い gate / 遅い gate の二段**: implement ノード出口 = 速い gate（影響ファイルをカバーするテストだけ、数分）。別の最終ノード = 遅い gate（フルスイート、数時間でも可、CI に投げて非同期でも harness は exit code を待つだけ）。
- **リグレッション**: 「全スイート green」＋「`count_non_decreasing` でテスト数が縮んでない」＋ skill ルール「バグ修正は必ず regression test を 1 本追加」。"以前通ってたテストが今も通る" ＝ "全スイート green" なので、本質はスイートが縮まないことの保証。
- **カバレッジ未達時**: gate が落ちる → skill が「未カバーの影響行に characterization test / 単体テストを書け」と指示 → *生成*は LLM の仕事（fat skill）、*強制*は gate（thin harness）。
- 「自動で担保」の意味 = 該当ノードを通過するには該当テストコマンドが exit 0 でなければ物理的に進めない、しかも harness 自身がコマンドを走らせるので嘘がつけない。これが §6 の traceability と組み合わさると「要件を足したら、その要件に紐づくテストが green でないと先へ進めない」になる。

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
- **コード知能インターフェース**: harness はフル索引器を内蔵しない（フル版の Python indexer は太い）。代わりに `find-symbol / refs / callers / implementers / show-symbol / outline / deps / rdeps / closure / impacted-by / tested-by` というコマンドを提供し、**実装はプロジェクトが設定したバックエンド**（LSP サーバへの小さいブリッジ or Serena/MCP 経由、または事前構築 SCIP/LSIF 索引、または tree-sitter）。能力は一級市民、実装は差し替え可能。ナレッジグラフは**キャッシュされた artifact（ファイル）であってコードではない**。`harness reindex` で外部索引器を叩いて再生成、stale 検出はグラフ vs git HEAD。
- **限界**: LSP/semantic は言語依存で不完全（マクロ・コード生成・動的ディスパッチ・リフレクション・ビルド時設定・多言語 repo）。静的グラフはよく拾うが動的エッジは漏れる ── blast radius の漏れと同じ穴、だから「フルスイート遅い gate」が安全網。事前グラフは陳腐化→再索引コスト（巨大 repo で時間かかるが、それは*マシン*が払うコストで*context*は払わない）。要約・パッケージカードはドリフト→再生成トリガが要る。

## 10. エージェント topology と context 構築

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
  - hook 隔離を失う→「危険な bash を block」等は harness のループ内 tool-call インターセプタとして再実装が要る（gate は既に in-process なので enforcement 本体は OK だが、ガード類は要移植）
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
  - **ファイル隔離 = 各 run に git worktree を 1 つ**（`--worktree` モード: `start` 時に worktree を作成し、その run の全 `cmd_exit_0`・編集はその worktree 内で行い、終了時に diff を取る）。複数 run = 複数 worktree = ファイル衝突なし。（`C:\ツール\git-worktree-runner` がこの用途のツール。）
  - **テスト内の共有外部状態（DB・ポート・ネットワーク）は worktree では解決しない** → テストが hermetic（per-run DB・動的ポート）であるか、共有リソースを使う run は直列化するしかない。harness は per-run scratch ディレクトリ/env を提供できるが完全には解決できない ──「並列 run にはテストの hermetic 性が要る」と文書化する。
  - **コードナレッジグラフ/索引は共有・read-mostly** → 並行読みは OK。`reindex` は atomic swap（temp に書いて rename）で並行読みと干渉しないようにする。
  - **API レート制限・コスト**: N 並列 = N 倍の Anthropic API 呼び出し ── 実用上の上限であって設計の欠陥ではない。
- **(b) 1 つの巨大変更を複数 harness で協調処理**（11.1 を粗い粒度で）: 「コーディネーター」harness が巨大変更を K 個の sub-change に分解し、K 個の子 harness run を spawn（各々が自分の worktree を持つ）、待つ、マージ。コーディネーターの `workflow.toml` に「fan-out ノード」（K 個の spec スライスから K 個の子 run を spawn）と「merge & validate ノード」（worktree をマージ、フルスイート実行、全 F-NNN にわたる `traceability_closed`）を書く。**10M ステップ変更を速くやる王道**: K 個の独立スライスに分解 → K 並列 harness run → マージ。前提は (a) と同じ（互いに素な blast radius、hermetic テスト or 直列化、worktree 隔離）。

### 11.3 これでまだ thin か（トレードオフ）

- 足すもののほとんどは**データ＋プリミティブ＋シェルアウト**: 並行可能性は `workflow.toml`（データ）の DAG/fork-join に書く / blast-radius 互いに素チェックは gate プリミティブ 1 個 / worktree 管理は git へのシェルアウト（`cmd_exit_0` と同程度の外部呼び出し）/ merge-validate ノードは `cmd_exit_0 "git merge ... && <フルスイート>"` の gate を持つただのノード。
- 本当に**新規の harness コード**になるのは: ①一個ずつでなく DAG ノードを並行実行するスケジューラ ②per-run worktree のライフサイクル管理。実体のある追加だが bounded。
- **確定**: fork/join はコアの `workflow.toml` モデルに入れる。ただし**並行実行は Phase 2**（並列スケジューラ＋per-run/per-branch worktree、ランタイム前提、§15）。**それまでの Phase 0/1 では `fork`/`join` ノードはブランチを逐次に実行する（degrade）** ── モデル上は DAG だが実行は逐次。並列性は当面 L3 sub-worker と「別々に起動した独立 run」（§11.2）で得る。

## 12. コマンド / CLI 表面

| コマンド | 引数 | 効果 | 状態を変えるか |
|---|---|---|---|
| `start "<intent>"` | intent | 新 run 開始、`start` イベント、status 出力 | 変える |
| `status` | `[--run R]` | 現在状態の表示（run_id, intent, 現ノード名と番号, 現ノードの skill 絶対パス, 出口 gate 一覧と各 pass/fail＋理由, 登録 artifacts, gate_evidence のキー, done か） | 変えない |
| `request-transition <to>`（別名 `advance` で次ノードへ） | `to`, `[--run R]` | 現ノードの出口 gate を全評価、全 pass なら `advance` イベント＋新 status、1 つでも fail なら `advance_rejected` イベント＋fail 一覧表示＋exit 1 | 変える（reject も記録） |
| `back "<reason>"` | `reason`, `[--run R]` | 前ノードへ、`back` イベント | 変える |
| `record-artifact <name> <path>` | `name`, `path`, `[--tag T]`, `[--run R]` | path 実在を確認、`artifact` イベント | 変える |
| `report-evidence <gate> <json\|@file>` | `gate`, `json`, `[--run R]` | json をパース、`gate_evidence` イベント | 変える |
| `ask "<質問>"` | `質問`, `--option ...`（2〜4 個）, `[--required]`, `[--run R]` | worker 向け。構造化質問を質問キューに積む（`question_queued` イベント。`required` 指定時は `no_pending_required_questions` gate がノードをブロック）。§13 | 変える |
| `questions` | `[--run R]` | 人間向け。保留中の質問（未回答エントリ）を一覧 | 変えない |
| `answer <question-id> <選択肢index\|"自由記述">` | `question-id`, `回答`, `[--run R]` | 人間向け。回答 → `human_answer` イベント。`kind=clarification` なら `spec.toml` の該当箇所を更新し `??` をクリア。§13 | 変える |
| `reset` | `[--run R] --yes` | `reset` イベント | 変える |
| `skill` | `[--run R]` | 現ノードの skill 内容/パス | 変えない |
| `spec <F-NNN>` | `F-NNN` | その要件と AC と紐づくテストだけ | 変えない |
| `gates` | `[--run R]` | 保留 gate 各 1 行 | 変えない |
| `outline <file>` | `file` | シグネチャだけのスケルトン | 変えない（semantic バックエンド委譲） |
| `show-symbol <sym>` | `sym` | そのシンボルの本体 | 変えない（semantic バックエンド委譲） |
| `find-symbol <name>` | `name` | シンボル位置 | 変えない（semantic バックエンド委譲） |
| `refs <sym>` | `sym` | 参照位置 | 変えない（semantic バックエンド委譲） |
| `callers <sym>` | `sym` | 呼び出し元位置 | 変えない（semantic バックエンド委譲） |
| `implementers <trait>` | `trait` | 実装位置 | 変えない（semantic バックエンド委譲） |
| `deps <module>` | `module` | 依存モジュール | 変えない（CKG バックエンド委譲、`docs/ckg.md`） |
| `rdeps <module>` | `module` | 逆依存モジュール | 変えない（CKG バックエンド委譲、`docs/ckg.md`） |
| `closure <sym> --depth N` | `sym`, `N` | 推移閉包（blast radius 候補） | 変えない（CKG バックエンド委譲） |
| `impacted-by <sym>` | `sym` | 変えたら壊れうる箇所 | 変えない（CKG バックエンド委譲） |
| `tested-by <sym>` | `sym` | カバーするテスト | 変えない（CKG バックエンド委譲） |
| `reindex [--full]` | `[--full]` | 外部索引器を叩いてコードナレッジグラフを再生成（既定はインクリメンタル、`--full` で全体、`docs/ckg.md`） | 変えない（キャッシュ artifact を更新） |
| `ckg-stale` | （なし） | コードナレッジグラフが git HEAD に対し陳腐化しているか | 変えない（CKG バックエンド委譲） |

注: 問い合わせ系のうち semantic（`find-symbol` 以降）は「コード知能バックエンドへの委譲」。

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

これで「人間 touchpoint の surface 機構」は**機構レベルで確定**: 質問キュー（`state/<run-id>.questions.jsonl`）＋ `harness ask / questions / answer` ＋ `no_pending_required_questions` gate ＋ 任意の通知。**残るオープン論点**（§16）: キューの上に載せる UI（CLI のみか、薄い TUI / web か）、実際のチャット面（Claude Code の AskUserQuestion / Slack 等）との統合をやるか。詳細な context 構築との関係は `docs/worker-context.md`、スキーマは `docs/schemas.md`。

## 14. 正直な限界（まとめて再掲）

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

## 15. 現状と次のステップ（段階制で確定）

- **現状**: `C:\ツール\thin-workflow-harness\src\*.rs` は v0 prototype（5 フェーズをコードにハードコード、gate を名前で match）。**この設計の方向に作り直す**（`phases.rs` を捨てて `workflow.toml` + プリミティブ gate に、`gates.rs` をプリミティブのみに）。

- **「CLI 版を中間に作るか」は解決した**: CLI 版 vs ランタイムは二者択一ではない。**CLI コアを先に作り、ランタイムはその厳密な superset を後で足す。同じ土台。CLI コアは捨てプロトタイプではなく以降の全フェーズの土台。** 以下の段階制で進める。

### Phase 0 — CLI コア

append-only イベントログ＋`derive_state`（§4）、プリミティブ gate 評価器（§7）、`workflow.toml` / `spec.toml` の config ローダー（§5・§6・`docs/schemas.md`）、`harness` コマンド表面（`status` / `request-transition` / `back` / `record-artifact` / `report-evidence` / `reset` / `ask` / `questions` / `answer` / 各種問い合わせ（`skill` / `spec` / `gates` / semantic クエリ群）/ `reindex` / `start`）。

- ノードは**厳密に逐次実行**: `workflow.toml` に `fork`/`join` があっても Phase 0 はブランチを逐次に実行する（並行実行は Phase 2、それまで逐次 degrade、§11.3）。
- エージェントは Claude Code が `harness` コマンドを叩く形（生 API spawn はまだ）。
- これで状態機械・gate・`workflow.toml`/`spec.toml` モデルを実人間込みで end-to-end 検証する。**ここで作るコアは throwaway ではない** ── 以降のフェーズはこの上に積む。
- 圧縮効果はこの段階では限定的（状態を抱えずクエリ／skill on-demand／grep より semantic／簡潔出力 ── per-node fresh context はまだ Phase 1）。

### Phase 1 — ランタイム層

生 Anthropic API で worker を spawn、context バンドル構築（`docs/worker-context.md` 準拠）、tool-call インターセプタ（blast radius 内に edit を制限）、prompt caching、worker ライフサイクル（§10）。これで per-node fresh context の圧縮が効く（§9・§10 の名前付き工夫）。

### Phase 1.5 — CKG バックエンド（Phase 1 と並行可）

`docs/ckg.md` 準拠の索引器連携を semantic コマンド（`find-symbol` / `refs` / `closure` / `impacted-by` / `tested-by` 等）に配線する。それまでこれらは LSP オンデマンドにフォールバック（or 未提供）。

### Phase 2 — 並列スケジューラ

`fork` ブランチの並行実行、`join`、ブランチごと worktree（＋独立 run 用の `--worktree`、§11.2）。ランタイム（Phase 1）が前提。それまで `fork`/`join` は逐次に degrade。

### skillify（`docs/skillify.md` 準拠）

opt-in の retrospective ノード＋playbook（再発する変更パターンを skill 化して再利用＝複利、§2・§5）。Phase 0 以降いつでも追加可（ほぼデータ＋1 ノード）。

## 16. オープンな論点（未定、要議論）

解決済みの論点（参照先）:

- 「CLI 版を中間に作るか」→ §15 段階制（CLI コア→ランタイム superset、同じ土台）
- 「CKG インクリメンタル更新の粒度」→ `docs/ckg.md`（ファイル単位＋逆依存閉包＋フルフォールバック。hot symbol の劣化は既知の限界）
- 「fork/join スケジューラをコアに入れるか」→ コアに入れる（並行実行は Phase 2、それまで逐次 degrade、§11.3）
- 「hermetic テストを強制する gate を置くか」→ 置かない（決定論的に hermetic 性を検出できない。並列 run の要件として文書化＋per-run scratch env、§11.2・§14）
- 「`characterize_before_change` をプリミティブにするか `cmd_exit_0` か」→ `characterize` ノードの出口 gate を `cmd_exit_0`（カバレッジチェック）にする（新プリミティブ不要、§7）
- 「skillify の仕組み化」→ `docs/skillify.md`
- 「`workflow.toml` の append 細則」→ §5 に細則を記載

残る論点:

- prompt caching の粒度（現状の最小版は `docs/worker-context.md` B4。最終確定は要検討）
- 人間 touchpoint：キュー上の UI（CLI のみか、薄い TUI / web か）、外部チャット面（Claude Code の AskUserQuestion / Slack 等）との統合をやるか
- 多言語 repo での CKG マージの細部（`docs/ckg.md` も触れるが詳細未確定）
- playbook の「再発」検出ヒューリスティックの調整（`docs/skillify.md`）
- tool-call インターセプタの具体（blast radius 制限の実装方法、Phase 1）
- worktree 隔離の粒度（run ごと？ ブランチごと？、§11.2）
- 並列ブランチ sub-log のマージ方式（§11.1）
