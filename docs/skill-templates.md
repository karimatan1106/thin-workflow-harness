# docs/skill-templates.md — カノニカル skill 文面案

> **注記**: これらは `skills/*.md` の確定文面の*案*である。実装リワーク時（`DESIGN.md` §15）に `skills/` 配下へ配置される想定。現 `skills/*.md` は v0 prototype なので、これと内容が一致しない。`DESIGN.md` §2（fat skills）/§6（spec）/§9（context）/§10（worker）と合わせて読むこと。
>
> 文面は実際にデプロイされる体（worker が読む本文）で書いてある。シナリオはハードコードせず、引数化・汎用化してある（`DESIGN.md` §2 の "引数化必須" 原則）。各 skill 30〜60 行目安。

---

## 全 skill 共通の前提（各 skill 本文がこれを織り込む）

- お前は thin workflow harness の worker。ワークフローのちょうど 1 ノードを担当する。状態は harness が所有しており、お前は書けない ── できるのは遷移リクエストと根拠提出だけ。
- 進め方: `harness status` で現ノードの保留 gate を確認 → それを満たす作業をする → `harness request-transition <next>`。却下されたら `advance_rejected` の `failed_gates` の理由を読んで直し、もう一度 `request-transition`。
- 壁打ち / 質問の作法（重要）: 人間に確認が要るときは自由記述で聞かず、構造化質問でキューに積む ── `harness ask "<質問>" --option "<選択肢A>" --option "<選択肢B>" [--option ...]`（選択肢 2〜4 個。自由記述 other は自動で付く。AskUserQuestion 方式）。回答が来るまでそのノードは進められない（`no_pending_required_questions` gate）。
- 「決めたら spec に書いて忘れる」: 壁打ちで 1 点決まったら即 `spec.toml` に反映 → context から落とす。後で要れば `harness spec <F-NNN>` で取り戻す。context はワーキングセットであって履歴ではない。
- ツールスコープ: このノードで使えるツールは `workflow.toml` が決める（research は read + semantic クエリ、edit なし / implement は read + edit（blast radius 内）+ run-command など）。無いツールを前提に作業を組み立てない。
- 禁止: フェーズ / ノードのスキップ。状態ファイル（イベントログ・`spec.toml` の他人が書く箇所）の直接編集。禁止語（`TODO` `TBD` `WIP` `FIXME` `未定` `未確定` `要検討` `検討中` `対応予定` `サンプル` `ダミー` `仮置き`）を成果物に残すこと。

---

## skill: research

このノードのゴール: 改修対象を理解し、検証可能な `spec.toml` を作って人間の承認を取る（What）。コードは編集しない。

やること:

1. 生の intent（`harness status` に出る）から、改修の影響範囲を semantic クエリで調べる ── `harness find-symbol <name>` / `harness refs <sym>` / `harness callers <sym>` / `harness outline <file>` / `harness closure <sym> --depth N`。grep は使わない（位置でなくテキストの塊が返り context が膨らむ）。本体を読むのは実際に触りそうな少数シンボルだけ（`harness show-symbol <sym>`）。
2. `spec.toml` を作る:
   - `F-NNN`（要件、1 行〜数行）。各要件に `files`（影響範囲＝blast radius のファイル一覧）と `tests`（その要件を検証する test コマンド）を紐づける。
   - `AC-N`（受入基準）。各々が `requirement`（F-ID）に紐づき、自分を検証する `test` コマンドを 1 つ持つ。
   - `invariant`（INV-N、改修で維持すべき不変条件）。これも `test` 化する。
   - `open_question`（未解決点）。本文中の `??` マーカーでもよいが、構造化して書くほうがよい。
3. 曖昧さに当たったら自由記述で悩まず `harness ask "<質問>" --option ... --option ...` でキューに積む。回答は harness が `spec.toml` の該当要件に書き込む。`open_questions_zero` gate は全部回答され `??` が無くなるまで fail。
4. 最後に spec の承認を取る: `harness ask "この spec はあなたが欲しい変更か?（要約: ...）" --option "承認" --option "修正が要る"`。「承認」が返ったら `harness report-evidence human_approval '{"verdict":"approved"}'`。
5. `harness record-artifact research_notes <path>`（調査メモ。semantic クエリで得た blast radius・依存・判断根拠を蒸留したもの）。

完了条件（gate）: `research_notes` が記録済み / `open_questions_zero` が pass（未解決点ゼロ・`??` なし）/ `json_has`（`human_approval` evidence の `verdict` が `approved`）が pass / `blast_radius_declared` が pass（各 F-NNN に `files` ≥1）。満たしたら `harness request-transition plan`。

---

## skill: plan

このノードのゴール: `spec.toml` に従い、実装計画を `plan` artifact（≤200 行）にまとめる（What）。コードは編集しない。

やること:

1. `harness status` と `harness spec <F-NNN>` で spec スライスを確認。必要なら `harness outline` / `harness deps` / `harness closure` で構造を再確認（本体は読まない、形だけ）。
2. `plan` artifact（≤200 行）を書く:
   - 変更ファイル一覧と各々の責務 / 新規ファイル一覧と各々の責務
   - 変更の順序（依存順）
   - 各 `F-NNN` をどのファイルのどの変更で実現するか
   - テスト方針（どの AC をどの test コマンドで担保するか、characterization test が要る箇所）
3. 計画が大きければ decomposition する: `F-007` を blast radius が互いに素な `F-007.1`（ファイル A,B）/ `F-007.2`（ファイル C,D）に分解 → `spec.toml` に追記し、`workflow.toml` に並列ノード（`fork` / `join`）を追加する。plan ノードは `can_append=true` なので `workflow.toml` に新規ノード（`fork`/`join` 含む）を追加できる ── ただし `workflow_append_only` の範囲内でのみ（既存ノード・既存 gate を弱められない／変えられるのは未到達ノードへの配線追加だけ／新規ノードは `[meta].mandatory_gates` を満たすこと）。判断に迷うときは `harness ask` で確認。
4. `harness record-artifact plan <path>`。

完了条件（gate）: `plan` artifact が登録済み / `max_lines plan.md 200` が pass / `traceability_closed` が pass（各 F-NNN に artifact ≥1 と exit 0 test ≥1、orphan なし）/ `workflow_append_only` が pass。満たしたら `harness request-transition implement`（`characterize` ノードがあるならそこへ）。

---

## skill: characterize

このノードのゴール: implement に入る前に、改修の影響ファイルのカバレッジが閾値未満なら characterization test（今の挙動を固定するテスト）を書いて閾値を満たす（What）。プロダクションコードのロジックは変えない ── 足すのはテストだけ。

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
2. 編集は blast radius 内のファイルだけ（宣言外パスへの書き込みは harness のインターセプタが拒否する）。
3. characterization test は「今の挙動を固定」するもの ── 「今の挙動が正しい」とは言っていない点に注意。改修意図に「そのバグも直す」が無い限り現状維持が安全。
4. 実装したソースを `harness record-artifact impl:<短い識別子> <path>` で登録（複数可）。新規ファイルは `--tag new`、既存改修ファイルは `--tag legacy`。新規ファイルは ≤200 行。既存改修ファイルは行数を増やさない（`lines_not_increased`）。
5. 行き詰まったら `harness ask` で確認。spec の前提が崩れていたら `harness back "..."` で research へ。

完了条件（gate）: `impl:` prefix の artifact が ≥1 登録済み・全て実在 / `no_regex`（禁止語が src に無い）/ 速い test gate（例 `cmd_exit_0 "cargo test --lib"`）が pass / tag ごとの追加 gate（new → `max_lines 200`、legacy → `lines_not_increased`）。満たしたら `harness request-transition test`。

---

## skill: test

このノードのゴール: 単体 / 結合 / E2E / カバレッジを走らせ、改修の影響範囲をカバーするテストが green であることを根拠付きで報告する（What）。

やること:

1. `harness status` で保留の test gate を確認（`cmd_exit_0` 系がノードに配線されている）。
2. テストを走らせる。落ちていたら `harness back "テスト失敗: <要点>"` で implement へ戻す（直接編集で取り繕わない）。
3. バグ修正が含まれるなら regression test を 1 本追加する（テスト総数を減らさない ── `count_non_decreasing` で縛られている）。
4. 影響範囲をカバーするテストが green であることを報告: `harness report-evidence test_result '{"command":"...","exit_code":0,"covered_count":N}'`（`covered_count` ＝影響行/シンボルをカバーしたテスト数）。

完了条件（gate）: 結合スイート（例 `cmd_exit_0 "cargo test --test '*'"`）が pass / E2E（例 `cmd_exit_0 "./e2e.sh"`）が pass / カバレッジ（例 `cmd_exit_0 "cargo llvm-cov --fail-under-lines 95"`）が pass / `count_non_decreasing`（テスト数が縮んでいない）が pass / `test_result` evidence が記録済み。満たしたら `harness request-transition review`。

---

## skill: review

このノードのゴール: 成果物を自己レビューし、問題なければ承認、あれば差し戻す（What）。

やること:

1. spec の各 `AC-N` が満たされているか確認（必要なら `harness spec <F-NNN>`、`harness outline` / `harness show-symbol` で diff の形を見る ── 本体を全部読まない）。
2. `invariant` が破れていないか / 禁止語が成果物に残っていないか / traceability が閉じているか（各 F-NNN に実在 artifact と exit 0 test、orphan code なし）を確認。
3. 問題なし → `harness report-evidence review '{"verdict":"approved","notes":"..."}'`。
4. 問題あり → 該当する前ノードへ `harness back "<理由>"`（AC 未達 → implement、spec の取りこぼし → research など）。

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
