> DESIGN.md の補助。設計の方針であって最終確定ではない部分も含む。

# example-walkthrough — 10M行変更の end-to-end トレース（worked example）

`docs/host-capabilities.md` の能力と `docs/operations.md` の運用面が実際にどう噛み合うかを、具体例で示す。設計の検証も兼ねる。

## 設定

例: 10M行の Web アプリ（`docs/target-codebase-structure.md` 風、`domains/identity/` 等、Rust + TS モノレポ、CKG = rust-analyzer SCIP）に「ログインに TOTP の 2FA を追加」。`.harness/` は onboarding 済み、`workflow.toml` = research/scope → plan → characterize → implement → test → security → review。

## トレース（各ノード: 何が起きるか / worker の context のおおよそのサイズ）

### Node 1 — research/scope（= 壁打ち）

worker spawn、初期 context ≈ {research skill ~40行 ＋ intent 1行 ＋ status ~10行} ≈ ~3k tokens、コードまだ無し。

- 意図言い直し → `harness ask` → 人間が修正（「org ポリシーで 2FA 必須のケースも要る」）。
- scope:
  - `find-symbol login`
  - `closure <login_handler> --depth 2`
  - **署名が変わるシンボルには `impacted-by` も**（← 重要、後述の弱いリンク #1）
  - `outline domains/identity/auth/...`
  - → CKG が ~30 ファイル位置 ＋ ~15 のアウトライン（~80行）→ context ~7k。
- `requirement.files` ドラフト → `harness ask`「漏れは?」→ 人間「policy enforcer が漏れてる」。
- `spec.toml` ドラフト: F-2FA、AC-1..6（各 test コマンド）、INV-1〜3、`requirement.files`(~20)、`??`（リカバリコード使い捨て? 何個? / org-policy 2FA は猶予期間?）→ `harness ask` → 人間回答 → spec 更新。
- 分解: F-2FA.1 = TOTP secret 保存、.2 = enrollment（＋UI）、.3 = login 検証、.4 = recovery codes、.5 = org-policy 統合。
- blast radii: .1 / .2 / .4 はほぼ互いに素（が registry/manifest ファイルを共有しうる — 弱いリンク #2）、.3 と .5 は `login.rs` / `enforce.rs` 共有 → 並列化不可。
- workflow 拡張: characterize → fork{.1, .2, .4} → join → .3 → .5 → test → security → review（`workflow_append_only` ✓）。
- spec 全体提示 → `harness ask` 承認 → `report-evidence human_approval` → `open_questions_zero` ✓ ＆ `json_has human_approval verdict eq approved` ✓ → frozen → advance。

**コスト ~30-80k tokens**（一番対話的）— 10M行に対して微小。

### Node 2 — plan（徹底的）

fresh worker、context ≈ {plan skill ＋ frozen spec.toml ~60行 ＋ status ＋ workflow.toml} ≈ ~5k、コード本体無し（plan は spec ＋ アウトラインで動く。Phase 0 では plan モードに入る）。

- `plan` artifact（≤200行）: 各 sub-req の作成/変更ファイル・責務・順序・テスト方針・AC↔test 対応・リスク・代替案。
- `harness record-artifact plan ...` → exit gates: `artifact_registered plan` ✓ `max_lines plan 200` ✓ `traceability_closed` ✓ `workflow_append_only` ✓ **`json_has plan_approval verdict eq approved`**（`harness ask`「この plan で進める? [承認/修正]」— 人間チェックポイント2つ目）✓ → advance。
- **plan が間違ってたら**（弱いリンク #3）: 「plan が良いか」gate は無い（L5 禁止）— 下流の implement で worker が本体を読んで infeasibility に気づき `harness back` で self-correct（多少の無駄）、または test で発覚、または（高ステークスなら）plan-approval で人間が気づく。

### Node 3 — characterize

blast-radius ファイルのカバレッジ確認 `cmd_exit_0 "cargo llvm-cov --affected ... --fail-under-lines 80"` → `login.rs` が 60%（古い）→ fail → worker が未カバー分岐に characterization test を書く（`show-symbol login_handler` で本体 ~150行 ＋ `tested-by login_handler`）→ 再実行 85% → pass → advance。

context ≈ ~10k ＋ 編集ターン。**characterization test は現在挙動を固定 — 現在のバグごと固定する**（弱いリンク #4）。

### Node 4 — fork {.1, .2, .4}（Phase 2 で並列、Phase 0/1 は逐次）

各ブランチ fresh worker、context ≈ {implement skill ＋ *その sub-req の* spec スライス ~30行 ＋ blast-radius ファイルのアウトライン ＋ 直接依存 ~50行 ＋ 作成/変更シンボルの本体 ＋ status ＋ plan スライス} ≈ ~6-15k。

- 新ファイル作成 → `record-artifact impl:totp-secret-store ... --tag new` → fast test `cmd_exit_0 "cargo nextest run -p identity totp_secret::"` → `report-evidence test_result`。
- exit gates: `artifact_registered impl:` ✓ `max_lines`（`--tag new` は ≤200）✓ `no_regex`（禁止語）✓ harness が**テストコマンドを再実行**して exit 0 確認 ✓ `cmd_exit_0 "cargo check --workspace"`（**workspace 全体** — domain をまたぐ署名 break をここで捕まえる、弱いリンク #1 の安い proxy）✓ → advance。
- **registry/manifest ファイル**（`wire.rs`・migration マニフェスト・`mod.rs` 再 export リスト）を複数ブランチが触ると並列化の毒（弱いリンク #2）— disjoint を証明できないなら `blast_radius_disjoint` が並列化を拒否 → 逐次実行。

### Node 5 — join

3 ブランチ worktree を `git merge --no-ff`（逐次なら no-op）→ 結合テスト `cmd_exit_0 "cargo nextest run -p identity"` ＋ **UI も触ったので `cmd_exit_0 "pnpm --filter identity-ui test"`**（← test/join ノードの gate は blast radius の言語/パッケージから導出すべき、弱いリンク #5）＋ `cmd_exit_0 "cargo check --workspace"` → exit gates: 結合 `cmd_exit_0` ✓ `traceability_closed` ✓ `count_non_decreasing` ✓ → advance。

### Node 6 — F-2FA.3 login 検証

`login.rs` 変更 ＋ `verify_totp.rs` 新規。context ≈ {implement skill ＋ F-2FA.3 spec スライス ＋ `login.rs` 本体 ~150行 ＋ `verify_totp.rs`（空）＋ 依存アウトライン ＋ `login.rs` の characterization test ~40行（壊すな）＋ status ＋ plan スライス} ≈ ~10-18k。

- `login.rs` が 200行 超える? → `login.rs` は変更既存ファイル ＝ `--tag legacy` → `lines_not_increased` 適用 → 2FA 分岐をインライン化すると増える → fail → worker は新ロジックを `verify_totp.rs` に**抽出**、`login.rs` の変更は数行（`if needs_2fa { return ... }`）に → ✓（gate が良い factoring を強制、弱いリンク #6 = gate が意図通り）。
- **INV-3（org-policy enforcement 維持）の characterization test が落ちる**（org-policy login が 2FA を要求するようになった）— これは意図した変更（人間承認済み）→ worker は「この characterization test は AC と衝突」と認識し AC のテストに**置換**、`traceability_closed` が新テストは F-2FA.5 に traced と確認（弱いリンク #4 のメカニズム）。
- advance → **Node 7 = F-2FA.5**（`enforce.rs` 変更、.3 の `verify_totp` 依存、機械的）→ advance。

### Node 8 — test

全 AC テスト ＋ identity crate ＋ UI テスト ＋ 遅いフルスイート。

- フルスイートが regression 検出: `domains/billing/invoice/service_account_login.rs` が `login_handler` を呼んでて F-2FA.3 が署名を変えた（`totp_code: Option<String>` 追加）→ billing がコンパイルしない/テスト落ちる。← blast radius が不完全（弱いリンク #1）— scope で `impacted-by login_handler` を走らせていれば caller が見えていた。実際は workspace 全体 `cargo check`（Node 4/6 の mandatory gate）がもっと早く捕まえる — つまりこの regression は本来 Node 6 で発覚すべき。
- `harness back` で .3 へ（caller を機械的修正、.3 の blast radius に追加 — `traceability_closed` が新規含有ファイルにテストも要求）または spec amendment（「service-account login は 2FA 不要」が新要件なら）→ 修正 → 再 advance。

### Node 9 — security（新ノード）

skill = `/security-review` の手順を移植（認証/認可・入力検証・インジェクション・シークレット・暗号・依存脆弱性・SSRF/path traversal 等）— Phase 0 では `/security-review` を invoke、Phase 1 ではこのチェックリスト。

- exit gates: `cmd_exit_0 "cargo audit"` ✓ `cmd_exit_0 "gitleaks detect --no-git --redact"`（エージェントがソースにシークレットを書いてないか）✓ `cmd_exit_0 "semgrep --config auto --error"`（あれば）✓ `evidence_recorded security_review`（構造化 findings）✓ 高リスク変更には `harness ask` で人間 sign-off → advance。
- 2FA は認証変更なので特に: TOTP secret の保存は暗号化されてるか、リプレイ攻撃対策（使用済みコードの再利用拒否）、recovery code は使い捨てか、ブルートフォース対策（コード検証のレートリミット）— これらは AC/invariant にも入ってるべきだが security ノードが最終確認。

### Node 10 — review

skill = 既存の自己レビュー（全 AC に passing test・不変条件・禁止語・traceability）＋ `/review` の手順を移植（命名・エラーハンドリング・エッジケース・テストの質・パフォーマンス・可読性）— Phase 0 では `/review` を diff に invoke。

`report-evidence review {verdict:approved}` → done。成果物: 全変更入りブランチ、人間がマージ（または最終ノードが `cmd_exit_0 "gh pr create"`）。

### 総コスト

~10 ノード × ~50-150k tokens/ノード ≈ ~0.6-1.7M tokens（2FA 程度、10M行アプリ）。~$2-25。**変更規模にスケールする、コードベース規模にはスケールしない**（harness は10M行を一度も読まない）— これが「圧倒的に少ない context」の実現。

## 弱いリンクのまとめ（この演習の収穫）

1. **blast radius の不完全さ（支配的）** — 人間の知識 ＋ CKG `impacted-by`（scope skill が*署名が変わるシンボル*に積極的に走らせる）＋ workspace 全体 `cargo check`（安く・早く）＋ 遅いフルスイート gate（遅く・高く）がバックストップ。動的ディスパッチ / config 配線依存はフルスイートのみ、かつテストがある場合のみ。
2. **registry/manifest ファイルでの並列化の衝突** — disjoint を証明できないなら並列化しない（保守的）。速度は*別 domain* の並列化から。任意の精緻化: fork 前に CKG で `references` / `imports` エッジをチェックして警告。
3. **悪い plan が implement/test まで捕まらない** — 「plan が良いか」gate は無い（L5 禁止）。spec が人間レビュー済みの契約、plan はアプローチ、悪いアプローチは `harness back` で self-correct。エスケープハッチ: plan-approval gate（デフォルトで入れる）。
4. **characterization-test と AC-test の境界が微妙** — 意図した変更で落ちる characterization test は AC test に置換（traceable）／意図しない副作用なら revert。harness は判別できない（L5）— worker が決め `traceability_closed` が結果の一貫性をチェック。implement skill が明示すべき:「characterization test を黙って編集するな、副作用を revert か AC にせよ」。
5. **test/join ノードの gate は blast radius の言語/パッケージから導出すべき**（Rust + TS → `cargo nextest && pnpm test`）、一度ハードコードでなく。
6. **レガシーファイルが変更で現在サイズ超** — `lines_not_increased` が抽出を強制（gate が意図通り、弱いリンクでない）。
