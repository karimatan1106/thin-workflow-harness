# docs/worker-context.md — worker context 構築仕様

> **注記**: 本ドキュメントは「harness がノード N の worker を spawn するとき、何を context として渡すか」の仕様。`DESIGN.md` §9（context 圧縮）/§10（topology・層モデル）/§11（並列）と合わせて読むこと。実装時に細部は調整される。

---

## B1. 構成要素

worker の context は **(a) 静的 system prompt**（worker 間でほぼ不変・prompt cache 対象）と **(b) 初期 user メッセージ**（ノードごと・前半キャッシュ可 / 後半可変）の 2 つから成る。

### (a) 静的 system prompt（短い・worker 間でほぼ不変・キャッシュ対象）

要旨（実際の文面はこれくらいの長さ）:

> お前は thin workflow harness の worker。ワークフローのちょうど 1 ノードを担当する。ツールは固定セット（このメッセージの後に渡される）。状態は harness が所有しており、お前は書けない ── できるのは遷移リクエストと根拠提出だけ。ノードの作業が完了したと思ったら `harness request-transition <next>` を呼ぶ。フェーズ / ノードをスキップするな。要件を発明するな。禁止語を成果物に残すな（`TODO` `TBD` `WIP` `FIXME` `未定` `未確定` `要検討` `検討中` `対応予定` `サンプル` `ダミー` `仮置き`）。状態が要るときは `harness status` で取れ ── context に状態を持ち越すな。使える harness コマンド: `status / skill / spec / gates / record-artifact / report-evidence / request-transition / back / ask`（＋このノードで許可された semantic / file / run-command 系）。

渡さないもの: `CLAUDE.md`、skill manifest、MCP ツール一覧、hooks / rules。これらは Claude Code の ambient であり、生 API spawn の worker には不要（`DESIGN.md` §10 の "工夫 3"）。

### (b) 初期 user メッセージ（ノードごと・前半キャッシュ可 / 後半可変）

1. **ノードヘッダ**: ノード id、種別（`research` / `plan` / `characterize` / `implement` / `test` / `security` / `review` / `fork` / `join`）、`skills/<N.skill>` の本文そのもの。
2. **spec スライス**: このノードが `serves` する `F-NNN` とその `AC-N`（各々に検証 test コマンド付き）、`invariant`、`files`（blast radius のファイル一覧）。── research / spec ノードの場合は spec が未だ無いので、代わりに「生の intent ＋ `spec.toml` を作れ」という指示（生 intent は `[meta].intent` 相当）。
3. **コード context**（ノード種別と `workflow.toml` の `context` 指示に従って harness が事前計算する）:
   - implement: blast radius のファイルのアウトライン（シグネチャ＋docstring）＋その直接依存のアウトライン＋編集対象シンボルの本体（判明していれば。判明していなければ「`harness show-symbol <sym>` で取れ」という指示）。
   - research / spec: 事前計算なし（worker が semantic クエリで探索する。`context = "none"` 相当）。
   - `context = { include = ["none"] }`: 何も付けない。
4. **コンパクト status**: 現ノード X/Y、保留 exit gate を各 1 行（現 pass / fail ＋ fail 理由）、登録済み artifacts（名前 → パス）、記録済み evidence キー。
5. **直近フィードバック**: 再 spawn（reject 後）の場合のみ ── 直前の `advance_rejected` イベントの `failed_gates`（gate ＋ 理由）。初回 spawn では空。

### 渡すツール（`workflow.toml` の `tools` に従う）

- **常時**（全ノード）: `harness status / skill / spec / gates / record-artifact / report-evidence / request-transition / back / ask`。
- **semantic**（許可されれば）: `harness outline / show-symbol / find-symbol / refs / callers / implementers / deps / rdeps / closure / impacted-by / tested-by`。research ノードは典型的にこれら全部。
- **ファイル操作**（許可されれば、例 implement ノード）: read、edit / write。**edit / write は blast radius 内のパスに制限される** ── harness のループ内 tool-call インターセプタが、ノードの宣言外パスへの書き込みを拒否する（`DESIGN.md` §10 のトレードオフ「hook 隔離を失う」の対処）。
- **コマンド実行**（許可されれば）: worktree 内でのコマンド実行（実質 `cmd_exit_0` 相当の任意コマンド ── ただしテスト・ビルド用途を想定）。
- **渡さないもの**: 任意 bash（ノードが必要としても、必ずインターセプト経由）、web、その他 ambient な何か。ツールが少ない＝ツールスキーマの context が小さい＋判断点が減る＋誤操作できない（`DESIGN.md` §10 の "工夫 5"）。

---

## B2. 構築手順（harness コード、決定論的）

1. `workflow.toml` をロード、ノード N を引く。
2. `skills/<N.skill>` を読む。
3. N の `serves` から `spec.toml` の spec スライスを解決する（該当 F-NNN・AC・invariant・files だけを抜き出す）。spec が未だ無い研究ノードなら生 intent をそのまま使う。
4. N の `context` 指示に従ってコード知能バックエンドを叩き、アウトライン / closure 等を計算する（implement なら blast radius のアウトライン＋直接依存のアウトライン＋編集対象シンボル本体、`"none"` なら何も計算しない）。
5. イベントログを `derive_state` して、コンパクト status を組み立てる（現ノード X/Y、保留 gate 各 1 行 pass/fail、artifacts、evidence キー）。
6. 再 spawn（直前に `advance_rejected` がある）なら、その `failed_gates` を feedback として用意する。
7. 組み立て: `[静的 system prompt]` ＋ `[skill ＋ spec スライス ＋ コード context]` ←ここまでがキャッシュプレフィックス（ノード内では不変）＋ `[コンパクト status ＋ feedback]` ←可変サフィックス。
8. ツールセット = N の `tools`（＋常時 harness コマンド）。インターセプタに blast radius（N の `files` / `serves` から導出）を渡す。
9. 生 Anthropic API で worker を spawn する（tool-use ループ・prompt caching は harness 側で自前実装、`DESIGN.md` §10）。

---

## B3. done プロトコル

1. worker が「ノードの作業が完了した」と判断 → `harness request-transition <next>` を呼ぶ。
2. harness が N の exit gates を全評価する。
3. **全 pass**: `advance` イベントを commit、worker を despawn、次ノードの worker を B2 の手順で spawn。
4. **1 つでも fail**: `advance_rejected` イベント（`failed_gates: [{gate, reason}]`）を記録 → このノードの worker を**新しい context で再 spawn**する（worker が蓄積した思考は引き継がない ── 必要な決定はイベントログと登録 artifact が運ぶ。`DESIGN.md` §10 の "ノードごと clean start"）＋ feedback として `failed_gates` を付ける。
5. N の `on_reject = { after = K, goto = G }` で、reject 回数が K に達したら G へ遷移する。`goto = "__human__"` の場合は人間へエスカレ（`DESIGN.md` §13 / `docs/schemas.md` 参照 ── 質問キューに `kind: escalation` のエントリを積み、`no_pending_required_questions` gate がそのノードをブロックする）。

---

## B4. prompt cache レイアウト

- **キャッシュプレフィックス**（ノード内では不変）= 静的 system prompt ＋ skill 本文 ＋ spec スライス ＋ コード context。
- **可変サフィックス**（リトライごとに変わる）= コンパクト status ＋ feedback。
- 同一ノードで worker を何度 spawn し直しても、プレフィックス（特に skill 部分）はキャッシュヒットする。
- Anthropic の prompt cache は 5 分 TTL を意識する ── リトライは間を空けすぎない（reject → 再 spawn は速やかに）。長時間放置するノード（人間の回答待ち等）はキャッシュが切れることを許容する。

（参考: `DESIGN.md` §16 のオープン論点「prompt caching の粒度」── 本ドキュメントの版が現時点の最小限の妥当な版。実装時に再検討されうる。）
