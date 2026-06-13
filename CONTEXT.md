# CONTEXT — thin-workflow-harness ドメイン用語集

> grill-with-docs の用語集(純粋ドメイン語彙、実装詳細は持たない)。
> 形式は `docs/CONTEXT-FORMAT.md`。曖昧/過負荷の語が出た時に lazy に追記する。

## CLI サブコマンド (CLI subcommand)
`harness <name>` としてシェルから起動できるコマンド。clap の `Command` enum として宣言され、
argv をパースして対応する handler に dispatch される。例: `advance`, `status`, `record-artifact`。
**skill テンプレートが operator/agent に叩かせてよいのはこの集合のみ。**

## WorkerAction
ランタイム(API worker)が走らせる agent が**出力**する内部アクションで、ランタイムが適用する。
シェルコマンドではない。例: `EditFile`, `RequestTransition`。CLI サブコマンドとは別語彙であり、
**WorkerAction 名をシェルコマンドとして `harness <それ>` で叩くことはできない**。両者の混同が
本変更が是正するドリフトの根本原因。

## advance / request-transition
- **advance** = 現ノードの出口 gate を全評価し、全 pass なら次ノードへ進める CLI サブコマンド。
  **ノード引数を取らない**(次ノードは workflow.toml の `next` で決まる)。
- **request-transition** = 遷移を要求する内部 WorkerAction 名。**CLI サブコマンドではない**。
  skill が `harness request-transition <node>` と書いていたのは誤りで、正しくは `harness advance`。

## stuck
operator/agent が「現ノードでこれ以上進めない」と判断したときの**人間エスカレーション**。
現ノードの作業を中断(node_aborted)して人間に回す。`back`(前ノードへ戻る)や `abandon`(run 全体の
放棄=terminal)とは異なる別概念。

## artifact
ノードが生成した成果物の登録(name → path)。`record-artifact` で event log に記録され、
State に畳み込まれる。後続ノードはこれを参照して前段の成果(plan 等)を読む。

## traceability(追跡可能性)
各 requirement(F-NNN)が「artifact ≥1 + exit 0 test ≥1」を持ち、orphan な artifact が無い状態。
プロセス成果物(research/plan/impl サマリ)は機能要件に属さないため、束ね用のプロセス要件
(F-100)に紐づけて orphan 拒否を回避する。
