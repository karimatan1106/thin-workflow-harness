> DESIGN.md の補助。設計の方針であって最終確定ではない部分も含む。

# failure-modes — 失敗モードカタログ

「『エージェント（または状況）が X を間違える → 何が捕まえるか / いつ / どの Phase / 残存ギャップ』を系統的に。`docs/example-walkthrough.md` の弱いリンクも組み込む。」

## 編集・ファイル操作

| # | 失敗 | 何が捕まえるか・いつ | Phase | 残存ギャップ |
|---|---|---|---|---|
| 1 | blast radius 外を編集 | Phase1: tool-call インターセプタが即拒否 / Phase0: リアルタイム block 無し、登録 artifact なら `traceability_closed` の orphan チェック、未登録編集はテスト失敗 or review まで | 1=即 / 0=遅 | **Phase 0 の弱点**（インターセプタ無し、Phase 1 で閉じる） |
| 2 | 新規ファイル >200行 | `max_lines`（`--tag new` の artifact_tag gate）が advance で fail | all | — |
| 3 | レガシーファイルを現在サイズ超に肥大 | `lines_not_increased` が advance で fail → 抽出を強制 | all | — |
| 4 | 禁止語を成果物に残す | `no_regex`（禁止語パターン）が advance で fail | all | — |
| 5 | 消すべきでないファイルを削除（テストを消してスイートを pass にする等） | テストなら `count_non_decreasing` / traced コードなら `traceability_closed` / 依存先なら workspace `cargo check` | all | — |
| 6 | **シークレットを漏らす** | **決定論的検出は無い**。`secrets_glob` は既知シークレットファイルの context への読込を防ぐ（best-effort 予防）が、ソースに書かれたら検出しない | — | **実ギャップ** → `[meta].mandatory_gates` に `cmd_exit_0 "gitleaks detect --no-git --redact"`（or trufflehog）系を推奨追加（「ソースに書いた」は捕まる、「context→API」は事後不能） |

## テスト・検証

| # | 失敗 | 何が捕まえるか・いつ | Phase | 残存ギャップ |
|---|---|---|---|---|
| 7 | テストが通ったと*嘘*をつく | gate は申告を信じない: **harness 自身がテストコマンドを再実行**（`cmd_exit_0` が真の gate、`report-evidence test_result` はメトリクス/notes 用の補助） | all | （要明確化を DESIGN/operations に） |
| 8 | 通るが意味の無いテスト | `traceability_closed` は「テストがある」だけ、`cargo llvm-cov` は実行されることだけ。**mutation testing**（`cmd_exit_0 "cargo mutants --fail-on-survived"`）が「テストが落ちない＝検証してない」を殺す — 唯一の本当の防御、遅いので slow ノード gate | mutation 有効時のみ | **honest ギャップ: テストの意味は決定論的に検証不能**、coverage ＋ mutation で下限のみ |
| 9 | テストスイートが flaky | `.harness/known-flaky.txt`（リトライ or「人間確認要」）、リストに無ければ `harness stuck` → 人間が修正/追加/accept | all | ギャップでなく**摩擦点** |
| 10 | テストスイートが不十分/プロジェクトにテスト無し | `characterize` ノード ＋ カバレッジ gate が変更前にテストを書かせる、テストゼロなら gate fail → 確立するまで進めない | all | **harness は「テスト無いプロジェクト」を直せない**、テスト債務を払うまで最初の変更が遅い |

## ワークフロー・状態操作

| # | 失敗 | 何が捕まえるか・いつ | Phase | 残存ギャップ |
|---|---|---|---|---|
| 11 | フェーズ/ノードをスキップ | `skip` コマンド無し、遷移は `advance`（gate付き）と `back` だけ | all | 構造的に不可能 |
| 12 | 状態ファイルを直接書く | `write-state` 無し ＋ Phase1: インターセプタが拒否、改竄で不正イベント列なら `derive_state` がエラー / Phase0: skill 指示頼み | 1=構造的 / 0=指示頼み | **Phase 0 の soft spot**（Claude Code worker が `Write` できてしまう、Phase 1 で閉じる） |
| 13 | workflow.toml を編集して自分の gate を弱める | `workflow_append_only` が run 開始時との diff を計算、非加法的変更を拒否 | all | — |
| 14 | 事後に blast radius を拡張して無認可編集を含める | 拡張は許可（漏れ発見の正当手段）だが `traceability_closed` が新規含有ファイルにテストを要求、拡張は記録される | all | — |
| 15 | ループに陥る | per-node `budget` → `node_aborted{reason:budget}` → `on_reject` | all | — |
| 16 | 本当に詰まった | `harness stuck` → `node_aborted{reason:stuck}` → 人間エスカレ、自己申告しなくても #15 が捕まえる | all | — |

## spec・意図

| # | 失敗 | 何が捕まえるか・いつ | Phase | 残存ギャップ |
|---|---|---|---|---|
| 17 | **spec が間違ってた** | spec 承認で人間がレビュー（唯一の人間判断チェックポイント）、人間も見落としたら最終 diff/PR で「これじゃない」→ spec amendment | all | **根本的限界: harness は「人間が誤った spec を承認した」を検出できない** |
| 18 | blast radius が不完全 | workspace 全体 `cargo check`（安く・早く）or 遅いフルスイート（遅く・高く）、scope skill の積極的 `impacted-by` で緩和 | all | **残存: 動的ディスパッチ/config 配線依存はフルスイートのみ、かつテストがある場合のみ、無ければ ship** |
| 19 | エージェントが implement 中に spec に異議 | `harness back` で spec へ → 再壁打ち → 再承認、AC と違うものを作っても `traceability_closed` が AC テスト pass を要求 | all | — |

## 並列・マージ

| # | 失敗 | 何が捕まえるか・いつ | Phase | 残存ギャップ |
|---|---|---|---|---|
| 20 | 並列ブランチが重なるファイルを編集 | join の `git merge` で衝突 → join fail → エスカレ、`blast_radius_disjoint` が宣言された重なりなら並列化拒否、残存は未宣言の重なり | 2 | fork 前に CKG エッジチェックで警告 |
| 21 | 個別 green なブランチがマージで互いを壊す | join ノードがマージ結果に結合/フルスイートを再実行（"consensus ceiling"） | 2 | — |

## resilience・infra

| # | 失敗 | 何が捕まえるか・いつ | Phase | 残存ギャップ |
|---|---|---|---|---|
| 22 | runtime がノード途中でクラッシュ | 最後の commit イベントから resume、未完了ノードは fresh worker で再 spawn、worktree の部分編集を破棄 | 1 | — |
| 23 | `cmd_exit_0` gate がハング | gate タイムアウト → 「timeout after Ns」で fail | all | — |
| 24 | API レート制限/5xx | バックオフリトライ、尽きたら `node_aborted{reason:api_error}` → `on_reject` | 1 | — |
| 25 | コスト暴走 | `run_cost_budget` 超過 → 人間エスカレ | 1 | — |

## catch-all

| # | 失敗 | 何が捕まえるか・いつ | Phase | 残存ギャップ |
|---|---|---|---|---|
| 26 | 上記が予期しない巧妙で間違ったことをする | 遅いフルスイート gate が普遍的バックストップ、ただしその挙動を exercise するテストがあれば。無ければ ship、本番で発覚 | all | **harness は正しさのオラクルではない** |

## harness が*保証しない*もの（正直に）

- **(d) spec が真の意図を捉えてるか** — 人間の入力、spec 承認がその一点を担うが人間を正しくはできない。
- **(e) テストが完全か** — coverage gate で下限、AC↔test 必須で「意図の各項目にテストがある」まで、mutation で「意味があるテスト」の下限 — どれも証明でない、動的依存・テスト無しの経路は ship 可能。
- **(f) ノード内のエージェントのアプローチが最適だったか** — plan は人間レビュー gate がデフォルトでは無い…いや、ユーザー要望で plan-approval をデフォルトに入れたので人間が plan も見る — ただし plan の*中身の良さ*は L5 なので gate できない、悪いアプローチは `harness back` で self-correct。

## harness が*保証する*もの

- **(a) 壊れた中間状態は下流に伝播しない**
- **(b) "done" に到達したならその状態は宣言された全 gate を満たす**
- **(c) 人間のレビュー負荷は O(spec)**

## このカタログで浮いたギャップ

1. #6 シークレット漏洩 → `gitleaks` / `trufflehog` 系 `cmd_exit_0` を `[meta].mandatory_gates` に推奨追加。
2. #7 明確化 → harness がテストコマンドを再実行するのが gate、`report-evidence` は補助。
3. #1 / #12 Phase 0 の soft spot → インターセプタ無し、Claude Code worker の協力頼み、Phase 1 で閉じる、Phase 0 では Claude Code hook をボーナス enforcement に使える。
4. #8 テストの意味 → mutation testing を任意の slow ノード gate に。
5. #18 残存 → 動的依存はフルスイートのみ、システム全体のテストカバレッジが効く。
