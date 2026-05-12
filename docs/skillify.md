# skillify — 複数 run またぎの学習・複利

> `DESIGN.md` §2（fat skills / fat data）§5（ワークフローモデル）の発展。Phase 0 以降いつでも追加可（ほぼデータ ＋ 1ノード）。

## 1. 目的

複数 run をまたいだ学習・複利。Garry Tan の "skillify this" / "code as memory"。一度やった手順は再発見しない。改善1回が全 future run に効く。

## 2. skillify する対象（3種を区別）

### (a) 手順 skill

各ノードの markdown skill（「research フェーズのやり方」「implement フェーズのやり方」）── ワークフロー雛形。ほぼ固定、たまに改善（reject ループが起きたノードの skill を明確化する等）。

### (b) タスクパターン skill / playbook

再発する*種類の変更*（例「FR システムに新取引所を追加」）── パラメータ化された `workflow.toml` ＋ `spec.toml` の雛形 ＋ 手順 markdown ＋ "lessons" ファイル。

次回 `harness start --playbook add-exchange "<取引所名>"` で workflow.toml / spec.toml を雛形から具体化、壁打ちはずっと完成度の高いドラフトから始まる。

### (c) fat code の蓄積

役に立った決定論チェック（カスタム validator スクリプト）はプロジェクトに残し `cmd_exit_0` で参照、future playbook が使う ── "code as memory"。

## 3. いつ・どう作るか

run 完了後（review ノード通過後）、任意の **retrospective ノード**（`workflow.toml` に opt-in）: エージェントが run のイベントログ ＋ diff ＋ 起きた gate reject をレビューし提案する ──

- (a) 手順 skill の更新（あるノードの skill が不明瞭で reject ループが起きてたら）
- (b) この変更が再発パターンに見えるなら新規 / 更新 playbook
- (c) 新しい fat-code validator

人間が承認（AskUserQuestion 方式: 「playbook 化する? / skill X を更新? / 破棄」）→ 承認分が `playbooks/` / `skills/` 更新 / `validators/` に書かれる。

「再発」の判定: 人間が言うか、ヒューリスティック（harness が intent テキスト類似 / blast radius 重複 / workflow 形状類似な run を N 件検出 → 「3回やってますね、skillify?」と提案）。retrospective ノード自体が skillify の "gate"（人間承認がフィルタ）。

## 4. run 間で運ばれるもの

- 手順 skill `skills/*.md` ── 共有・ゆっくり進化
- playbook `playbooks/<name>.toml` ＋ 付随 `.md` ── パラメータ化雛形
- playbook ごとの **lessons log** `playbooks/<name>.lessons.md` ── append-only。playbook を使うたびに起きた gotcha / reject を追記（次回は事前に警告される）＝ **これが複利**、playbook が使うほど良くなる
- fat-code validators `validators/*` ── `cmd_exit_0` で参照

運ばれないもの: run のイベントログ（run 固有）、worker の思考（破棄）。

## 5. playbook の構造

- `playbooks/<name>.toml`:
  - `[params]` ── name, type, description のパラメータ宣言
  - `[workflow]` セクション ── 具体化されると `workflow.toml` になる、`$param` プレースホルダ
  - `[spec_template]` ── 具体化されると `spec.toml` の draft になる、F-NNN / AC-N の雛形に `$param` 埋め込み
- 付随 `playbooks/<name>.md`（手順説明）と `playbooks/<name>.lessons.md`（append-only 教訓）。
- `harness start --playbook <name> <param値...>` で具体化 → `workflow.toml` / `spec.toml` を生成 → 通常の run として進む（壁打ちは雛形ドラフトから）。

（注: playbook の TOML スキーマの詳細は実装時に確定 ── ここでは構造の方針のみ）

## 6. 正直な限界

- playbook が古くなる（コードベースが進化すると雛形が陳腐化）→ lessons log と、使うたびの retrospective で更新。
- 「再発」検出ヒューリスティックは荒い（偽陽性で余計な playbook 提案、偽陰性で見逃し）→ 最終判断は人間。
- playbook の過剰生成は逆にノイズ ── retrospective での人間承認がフィルタ。
- skillify は context を*減らさない*（むしろ playbook を読むコストが乗る）── ただし playbook は壁打ちの再発見を省くので正味は得、という想定。
