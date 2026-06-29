---
type: reference
title: "docs/skill-templates.md — カノニカル skill 文面案"
description: "> 注記: これらは skills/*.md の確定文面の*案*である。実装リワーク時（DESIGN.md §15、Phase 0）に skills/01-research.md〜skills/08-join.md として skills/ 配下へ配置される（連番）── 下の ## skill: <name> 見出…"
tags: [harness, docs]
---

# docs/skill-templates.md — カノニカル skill 文面案

> **注記**: これらは `skills/*.md` の確定文面の*案*である。実装リワーク時（`DESIGN.md` §15、Phase 0）に `skills/01-research.md`〜`skills/08-join.md` として `skills/` 配下へ配置される（連番）── 下の `## skill: <name>` 見出しとファイル名の対応は **research↔01 / plan↔02 / characterize↔03 / implement↔04 / test↔05 / security↔06 / review↔07 / join↔08**。現 `skills/*.md` は v0 prototype（番号が一部ズレる ── v0 の `04-test.md` ≠ 新 `04-implement.md` 等）なので、これと内容も番号も一致しない ── v0 は Phase 0 リワークで丸ごと置換される。標準は **research / plan / characterize / implement / test / security / review**（＋並列なら join）の 8 種。Phase 0（ホスト＝Claude Code）では各 skill がホストの組込み（plan モード／`/security-review`／`/review`／AskUserQuestion）を優先し、無ければ skill 内の手順に従う（`[meta].host`、`DESIGN.md` §10・`docs/host-capabilities.md`）。`DESIGN.md` §2（fat skills）/§6（spec）/§9（context）/§10（worker）と合わせて読むこと。
>
> 文面は実際にデプロイされる体（worker が読む本文）で書いてある。シナリオはハードコードせず、引数化・汎用化してある（`DESIGN.md` §2 の "引数化必須" 原則）。各 skill 30〜60 行目安。

---

## 全 skill 共通の前提（各 skill 本文がこれを織り込む）

- お前は thin workflow harness の worker。ワークフローのちょうど 1 ノードを担当する。状態は harness が所有しており、お前は書けない ── できるのは遷移リクエストと根拠提出だけ。
- 進め方: `harness status` で現ノードの保留 gate を確認 → それを満たす作業をする → `harness request-transition <next>`。却下されたら `advance_rejected` の `failed_gates` の理由を読んで直し、もう一度 `request-transition`。
- 壁打ち / 質問の作法（重要）: 人間に確認が要るときは自由記述で聞かず、構造化質問でキューに積む ── `harness ask "<質問>" --option "<選択肢A>" --option "<選択肢B>" [--option ...]`（選択肢 2〜4 個。自由記述 other は自動で付く。AskUserQuestion 方式）。回答が来るまでそのノードは進められない（`no_pending_required_questions` gate）。
- 詰まったら正直に申告: これ以上進めない（gate が満たせない・前提が崩れている・情報が足りない）と判断したら、`harness request-transition` を空打ちで繰り返さず `harness stuck "<理由>"` でエスカレせよ。harness が `node_aborted` を書いて人間に回す。
- 「決めたら spec に書いて忘れる」: 壁打ちで 1 点決まったら即 `spec.toml` に反映 → context から落とす。後で要れば `harness spec <F-NNN>` で取り戻す。context はワーキングセットであって履歴ではない。
- ツールスコープ: このノードで使えるツールは `workflow.toml` が決める（research は read + semantic クエリ、edit なし / implement は read + edit（blast radius 内）+ run-command など）。無いツールを前提に作業を組み立てない。
- 禁止: フェーズ / ノードのスキップ。状態ファイル（イベントログ・`spec.toml` の他人が書く箇所）の直接編集。禁止語（`TODO` `TBD` `WIP` `FIXME` `未定` `未確定` `要検討` `検討中` `対応予定` `サンプル` `ダミー` `仮置き`）を成果物に残すこと。
- **`report_evidence` の `gate` 引数は evidence の *key 名* を渡す**（gate プリミティブの *種別名* ではない）── 例: `workflow.toml` に `{gate="evidence_recorded", args={key="human_approval"}}` という出口 gate があるとき、worker は `report_evidence(gate="human_approval", json={"verdict":"approved"})` を呼ぶ。`gate="evidence_recorded"` は **誤り** ── `evidence_recorded` は gate プリミティブの種別名であって evidence key ではない。`json_has`/`evidence_recorded`/`count_non_decreasing` の `evidence_key`/`baseline_key` 引数の値こそが `report_evidence` の `gate` に入れる名前。

---

## skill: research

このノードのゴール: 改修対象を理解し、検証可能な `spec.toml` を作って人間の承認を取る（What）。コードは編集しない。**壁打ち / scope のループ**であり、唯一「無制限の人間対話が OK」な場所 ── ここで over-ask しろ、間違った実装より安い。決まったら即 `spec.toml` に書いて context から出す（spec は結晶化した壁打ち）。

順序:

1. **意図の言い直し**（最初、コードを読む前）── 生の intent（`harness status` に出る）を自分の言葉で言い直し、`harness ask "この理解で合ってる?（要約: ...）" --option "合ってる" --option "ずれてる（otherで補足）"`。
2. **scope（blast radius の発見）** ── `harness find-symbol <name>` / `harness closure <sym> --depth N` / **署名が変わるシンボルには `harness impacted-by <sym>` も積極的に**（変えたら壊れうる箇所＝references エッジ）/ `harness outline <file>`（アウトラインを読む ── 本体じゃない）で blast-radius 候補集合を作る。本体を読むのは実際に触りそうな少数シンボルだけ（`harness show-symbol <sym>`）。grep は使わない（位置でなくテキストの塊が返り context が膨らむ）。候補集合を `requirement.files` のドラフトにして `harness ask "blast radius はこれで漏れは? 触っちゃダメなのは?" --option "OK" --option "漏れ/禁止域あり（otherで)"`。
3. **不変条件の特定**（何を壊しちゃダメか）── 各々 `[[invariant]]`（INV-N）として書き、それぞれに `test` を紐づける。不確かなら `harness ask "X の挙動は維持すべき?" --option "維持" --option "変えてよい"`。
4. **受入基準** ── 各々 `[[acceptance]]`（AC-N、`requirement` に紐づく F-ID＋`test` 1 つ）。「all AC テスト green」≡「意図した変更」になる程度に具体的に書く。曖昧な AC は smell（テスト化できない AC は AC でない）。
5. **残った曖昧さ** ── `??` で `spec.toml` 本文に書き、`harness ask` で潰す。**決定を訊け、情報を訊くな** ── コードで分かること（「この関数はどこから呼ばれる?」等）は自分で見つけよ、人間に訊くのは判断（「どっちの方針にする?」）だけ。`open_questions_zero` gate は全部回答され `??` が無くなるまで fail。
6. **分解**（大きければ）── sub-requirement（F-007 → F-007.1 / .2）に割る。できれば互いに素な blast radius にする＝並列化可（`fork`/`join`）。対応するノードを `workflow.toml` に追加するのは plan ノードの仕事（plan が `can_append=true`）── research ではどう割るかを `spec.toml` に書くところまで。
7. **最終承認** ── spec 全体を提示して `harness ask "この spec、欲しい変更か?（要約: F-NNN/AC/不変条件/blast radius）" --option "承認" --option "修正が要る（otherで)"`。承認が返ったら `harness report-evidence human_approval '{"verdict":"approved"}'`。
8. `harness record-artifact research_notes <path>`（調査メモ。semantic クエリで得た blast radius・依存・判断根拠を蒸留したもの）。

完了条件（gate）: `research_notes` が記録済み / `open_questions_zero` が pass（未解決点ゼロ・`??` なし）/ `no_pending_required_questions` が pass / `json_has`（`human_approval` evidence の `verdict` が `approved`）が pass / `blast_radius_declared` が pass（各 F-NNN に `files` ≥1）。満たしたら `harness request-transition plan`。

---

## skill: plan

このノードのゴール: `spec.toml` に従い、**徹底的な**実装計画を `plan` artifact（≤200 行）にまとめ、人間の plan 承認を取る（What）。コードは編集しない。これは人間チェックポイント 2 つ目（spec 承認に続く、`DESIGN.md` §13）── heavyweight に。

ホストに plan モードがあるならそれを使う（`[meta].host = "claude-code"` のとき ── plan モードは read-only research を強制し、`ExitPlanMode` で計画を提示する。`docs/host-capabilities.md`）。無ければ以下の手順。

やること:

1. `harness status` と `harness spec <F-NNN>` で spec スライスを確認。必要なら `harness outline` / `harness deps` / `harness closure` で構造を再確認（本体は読まない、形だけ）。
2. **徹底的な** `plan` artifact（≤200 行）を書く:
   - 変更ファイル一覧と各々の責務 / 新規ファイル一覧と各々の責務
   - 変更の順序（依存順）
   - 各 `AC-N` ↔ それを担保する test コマンドの対応（漏れがあれば spec が不完全 ── research へ `harness back`）
   - リスク（壊しうる不変条件、隠れた依存、blast radius の漏れの可能性）
   - 代替案の検討（なぜこのアプローチか、却下した案と理由）
3. 計画が大きければ decomposition する: `F-007` を blast radius が互いに素な `F-007.1`（ファイル A,B）/ `F-007.2`（ファイル C,D）に分解 → `spec.toml` に追記し、`workflow.toml` に並列ノード（`fork` / `join`）を追加する。plan ノードは `can_append=true` なので `workflow.toml` に新規ノード（`fork`/`join` 含む）を追加できる ── ただし `workflow_append_only` の範囲内でのみ（既存ノード・既存 gate を弱められない／変えられるのは未到達ノードへの配線追加だけ／新規ノードは `[meta].mandatory_gates` を満たすこと）。判断に迷うときは `harness ask` で確認。
4. `harness record-artifact plan <path>`。
5. **plan 承認を取る**: `harness ask "この plan で進める?（要約: 変更ファイル/順序/AC↔test/リスク）" --option "承認" --option "修正が要る（otherで)"`。承認が返ったら `harness report-evidence plan_approval '{"verdict":"approved","notes":"..."}'`。

完了条件（gate）: `plan` artifact が登録済み / `max_lines plan.md 200` が pass / `traceability_closed` が pass（各 F-NNN に artifact ≥1 と exit 0 test ≥1、orphan なし）/ `workflow_append_only` が pass / `json_has`（`plan_approval` evidence の `verdict` が `approved`）が pass。満たしたら `harness request-transition characterize`（無ければ `implement`）。

---

## skill: characterize

このノードのゴール: implement に入る前に、改修の影響ファイルのカバレッジが閾値未満なら characterization test（今の挙動を固定するテスト）を書いて閾値を満たす（What）。プロダクションコードのロジックは変えない ── 足すのはテストだけ。**characterization test は現在挙動を固定する ── 現在のバグごと固定する**（改修の意図に「そのバグも直す」が無い限り、現状維持が安全 ── バグを直すのは implement で、そのとき AC が裏付けになる）。

このノードは任意（カバレッジが既に十分なら `workflow.toml` からこのノード自体を省いてよい）。置く場合は plan と implement の間。

やること:

1. `harness status` / `harness spec <F-NNN>` で担当 F-NNN と `files`（blast radius）を確認。
2. 未カバーの影響シンボル・行を特定する ── `harness outline <影響ファイル>` で構造を把握し、`harness tested-by <シンボル>` でそのシンボルをカバーしているテストを引く（ゼロ件・薄いものが characterization の対象）。
3. 対象に characterization test を追加する。**既存の挙動を assert するだけ** ── 「今の挙動が正しい」とは言っていない。改修の意図に「そのバグも直す」が明示されていない限り、現状の挙動が変に見えてもそのまま固定するのが安全（バグごと固定して、implement で意図的に直すときにテストを更新する）。
4. カバレッジツールを blast radius に対して走らせ、閾値（このノードの出口 gate が指定する `--fail-under N`）を満たすことを確認する。
5. 行き詰まったら `harness ask "<質問>" --option ... --option ...` でキューに積む。plan の前提が崩れていたら `harness back "..."` で plan へ。

完了条件（gate）: このノードの出口 `cmd_exit_0`（カバレッジツールを `--affected <blast_radius> --fail-under N` で実行し exit 0 ── ツール名・閾値はプロジェクト依存）が pass。満たしたら `harness request-transition implement`。

禁止: ノードのスキップ。状態ファイルの直接編集。禁止語を成果物に残すこと（共通の前提どおり）。

---

## skill: implement

このノードのゴール: `plan` に従ってコードを実装する（What）。

やること:

0. このノードの手前に `characterize` ノードがある場合がある（plan と implement の間）── そこを通過済みということは影響行のカバレッジが閾値を満たしている（足りなければそのノードで characterization test を書かされている）。`characterize` ノードが無いワークフローでも、影響行のカバレッジが薄いと感じたら自分で characterization test を足してよい。
1. `harness status` / `harness spec <F-NNN>` で担当する F-NNN・AC・`files`（blast radius）を確認。渡された初期 context に blast radius のアウトラインと触るシンボルの本体が入っているはず。足りなければ `harness show-symbol <sym>` / `harness outline <file>` で掘る。
2. 編集は blast radius 内のファイルだけ（宣言外パスへの書き込みは harness のインターセプタが拒否する）。**変更が*レガシーファイル*を現在サイズより大きくしそうなら、新ロジックを新ファイルに抽出せよ**（`lines_not_increased` gate がこれを強制する ── レガシーは増やすな、新規は ≤200 行）。
3. **characterization test が自分の変更で落ちたら**: (a) 意図しない副作用なら revert する。(b) 意図的な挙動変更なら、それは spec の AC でなければならない ── そうでないなら `harness back "..."` で research/spec へ戻って AC を足してこい。**characterization test を黙って編集するな**（現在挙動の固定を勝手に書き換えると、改修の意図が spec に残らないまま挙動が変わる）。
4. 実装したソースを `harness record-artifact impl:<短い識別子> <path>` で登録（複数可）。新規ファイルは `--tag new`、既存改修ファイルは `--tag legacy`。新規ファイルは ≤200 行。既存改修ファイルは行数を増やさない（`lines_not_increased`）。
5. このノードの出口 gate（または `[meta].mandatory_gates`）に `cmd_exit_0 "cargo check --workspace"` 等のビルドチェックが入っている場合がある ── 自分の編集でビルドを壊していないか確認してから `request-transition` せよ。
6. 行き詰まったら `harness ask` で確認。spec の前提が崩れていたら `harness back "..."` で research へ。**詰まったら `request-transition` を無理に呼ばず `harness stuck "<理由>"` で正直にエスカレせよ**（空打ちで gate を満たそうとしない）。

完了条件（gate）: `impl:` prefix の artifact が ≥1 登録済み・全て実在 / `no_regex`（禁止語が src に無い）/ 速い test gate（例 `cmd_exit_0 "cargo test --lib"`）が pass / tag ごとの追加 gate（new → `max_lines 200`、legacy → `lines_not_increased`）/（あれば）`cmd_exit_0 "cargo check"`。満たしたら `harness request-transition test`。

---

## skill: test

このノードのゴール: 単体 / 結合 / E2E / カバレッジを走らせ、改修の影響範囲をカバーするテストが green であることを根拠付きで報告する（What）。

やること:

1. `harness status` で保留の test gate を確認（`cmd_exit_0` 系がノードに配線されている ── テスト gate は blast radius の言語/パッケージから導出される、Rust+TS なら `cargo nextest && pnpm test` 等、`docs/operations.md` §1）。ビルドチェック（`cmd_exit_0 "cargo check --workspace"` 等）が出口 gate / `[meta].mandatory_gates` にある場合は、regression test を足したあとでビルドを壊していないかも確認する。
2. テストを走らせる。落ちていたら `harness back "テスト失敗: <要点>"` で implement へ戻す（直接編集で取り繕わない）。これ以上進めないなら `harness stuck "<理由>"`。
3. バグ修正が含まれるなら regression test を 1 本追加する（テスト総数を減らさない ── `count_non_decreasing` で縛られている）。
4. 影響範囲をカバーするテストが green であることを報告: `harness report-evidence test_result '{"command":"...","exit_code":0,"covered_count":N}'`（`covered_count` ＝影響行/シンボルをカバーしたテスト数）。

完了条件（gate）: 結合スイート（例 `cmd_exit_0 "cargo test --test '*'"`）が pass / E2E（例 `cmd_exit_0 "./e2e.sh"`）が pass / カバレッジ（例 `cmd_exit_0 "cargo llvm-cov --fail-under-lines 95"`）が pass / `count_non_decreasing`（テスト数が縮んでいない）が pass / `test_result` evidence が記録済み（**ただし `cmd_exit_0` 系は harness 自身が再実行するので `test_result` は補助の申告であって信頼の源泉でない**、`DESIGN.md` §7）。満たしたら `harness request-transition security`。

---

## skill: security

このノードのゴール: 最終的なセキュリティ確認（test の後・review の前）（What）。コードは編集しない（findings は report-evidence で報告し、修正が要れば前ノードへ差し戻す）。

ホストに `/security-review` skill があればそれを invoke せよ（`[meta].host = "claude-code"` のとき、`docs/host-capabilities.md`）。無ければ以下のチェックリスト:

- **認証 / 認可** ── 権限チェックの抜け、IDOR（他人の ID を渡すとアクセスできる）、特権昇格
- **入力検証** ── インジェクション（SQL / コマンド / path traversal）、安全でないデシリアライズ
- **シークレット** ── ハードコードされた認証情報、ログへの漏洩、暗号化されていない保存
- **暗号** ── 弱いアルゴリズム、ハードコードキー、不適切な乱数（CSPRNG でない）
- **依存脆弱性** ── `cargo audit` / `npm audit`（これは出口 gate で自動 ── 自分でも確認）
- **SSRF / path traversal** ── ユーザー入力が URL / ファイルパスに流れていないか
- **レートリミット** ── ブルートフォース対策（ログイン・トークン検証等）

findings を `harness report-evidence security_review '{"verdict":"clean_or_addressed","notes":"...","findings":[...]}'` で報告する（`verdict` は問題なしか、見つけたが前ノードで対処済みなら `clean_or_addressed`。未対処の問題があるなら該当前ノードへ `harness back "<理由>"`）。**高リスク変更（認証・決済・権限まわり）は `harness ask "<セキュリティ要約> ── sign-off する?" --option "OK" --option "差し戻す（otherで)"` で人間 sign-off を取れ**。これ以上進めないなら `harness stuck "<理由>"`。

完了条件（gate）: `cmd_exit_0`（`cargo audit` / `gitleaks detect --no-git --redact` / `semgrep` 等 ── プロジェクトの言語に応じ `harness init` の onboarding で設定）が pass / `evidence_recorded security_review` が pass（高リスク変更時は `json_has security_review verdict eq clean_or_addressed` も）。満たしたら `harness request-transition review`。

禁止: ノードのスキップ。状態ファイルの直接編集。禁止語を成果物に残すこと（共通の前提どおり）。

---

## skill: review

このノードのゴール: 成果物を自己レビューし、問題なければ承認、あれば差し戻す（What）。

ホストに `/review` skill があれば diff に対してそれを invoke せよ（`[meta].host = "claude-code"` のとき、`docs/host-capabilities.md`）。無ければ以下の観点で自己レビュー: 命名 / エラーハンドリング / エッジケース / テストの質 / パフォーマンス / 可読性 / 既存パターンとの整合。

やること:

1. spec の各 `AC-N` に passing test があるか確認（必要なら `harness spec <F-NNN>`、`harness outline` / `harness show-symbol` で diff の形を見る ── 本体を全部読まない）。
2. `invariant` が破れていないか / 禁止語が成果物に残っていないか / traceability が閉じているか（各 F-NNN に実在 artifact と exit 0 test、orphan code なし ── `traceability_closed`）を確認。
3. 上記の観点（命名・エラーハンドリング・エッジケース・テストの質・パフォーマンス・可読性・既存パターンとの整合）で diff を見る。
4. 問題なし → `harness report-evidence review '{"verdict":"approved","notes":"..."}'`。
5. 問題あり → 該当する前ノードへ `harness back "<理由>"`（AC 未達 → implement、spec の取りこぼし → research、セキュリティ懸念 → security など）。

完了条件（gate）: `review` evidence が `{verdict:"approved"}` / `traceability_closed` が pass。満たしたら `harness request-transition`（次が無ければそこで run 完了）。

---

## skill: join

このノードのゴール: 全並列ブランチがマージされた結果に対し、結合テスト/フルスイートを再実行し、全 F-NNN にわたって traceability_closed を確認し、ブランチ間で壊し合いが無いことを確認する（What）。`fork` ノードは worker 作業がほぼ無い（ブランチ起動の宣言だけ）ので skill を持たないが、`join` は再検証 worker が動く。

やること:

1. `harness status` で待ち合わせ対象のブランチが全部 done か確認する（まだなら待つ ── join の gate がブランチ未完了で fail する）。
2. ワークツリー/ブランチをマージする（`git merge --no-ff <branch_a> <branch_b> ...`）。コンフリクトがあれば解消する（ブランチの blast radius は互いに素なはずなので通常は出ないが、隠れた依存で出ることはある）。
3. フルスイート/結合テストを実行する。
4. 結果を `harness report-evidence`（join ノードの出口 gate が要求する evidence、例 `test_result` 等）で報告する。
5. 落ちていたら原因のブランチに対応するノードへ `harness back "<理由>"`。直接編集で取り繕わない。

完了条件（gate）: join ノードの出口 ── `cmd_exit_0`（`git merge ... && <フルスイート>`、別途 `<結合スイート>`）/ `traceability_closed`（全 F-NNN にわたって閉じている）/ `count_non_decreasing`（テスト数が縮んでいない）等が pass。満たしたら `harness request-transition <next>`（次が無ければそこで run 完了）。

禁止: ノードのスキップ。状態ファイルの直接編集。禁止語を成果物に残すこと（共通の前提どおり）。
