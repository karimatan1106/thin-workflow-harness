//! debug CLI ── clap derive とサブコマンド宣言。
//!
//! match dispatch は `cli_dispatch.rs` に分離（200 行制約）。
//! 「Claude Code が `harness` コマンドを叩く」前提。

use clap::{Parser, Subcommand};

use crate::cli_dispatch::dispatch;

#[derive(Parser)]
#[command(name = "harness", about = "thin workflow harness (Phase 0 walking skeleton)")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// 新 run を開始する。
    Start {
        intent: String,
        /// 作業ディレクトリ（worktree モード ── skeleton では scaffold、現状は受け取るだけ）。
        #[arg(long)]
        worktree: Option<String>,
    },
    /// run の状態を表示する。
    Status {
        #[arg(long)]
        run: Option<String>,
    },
    /// 現ノードの出口 gate を全評価し、全 pass なら次ノードへ進む。
    Advance {
        #[arg(long)]
        run: Option<String>,
        /// 作業ディレクトリ（worktree モード ── skeleton では scaffold）。
        #[arg(long)]
        worktree: Option<String>,
    },
    /// 前ノードへ戻る。
    Back {
        reason: String,
        #[arg(long)]
        run: Option<String>,
    },
    /// artifact を登録する。
    RecordArtifact {
        name: String,
        path: String,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        run: Option<String>,
    },
    /// gate evidence を記録する（json 文字列か @ファイル）。
    ReportEvidence {
        gate: String,
        json: String,
        #[arg(long)]
        run: Option<String>,
    },
    /// 構造化質問を質問キューに積む（worker 向け）。
    Ask {
        question: String,
        #[arg(long = "option")]
        option: Vec<String>,
        #[arg(long)]
        header: Option<String>,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        required: bool,
        #[arg(long)]
        run: Option<String>,
    },
    /// 保留中の質問を一覧する（人間向け）。
    Questions {
        #[arg(long)]
        run: Option<String>,
    },
    /// 質問に回答する（人間向け）。
    Answer {
        question_id: String,
        choice: String,
        #[arg(long)]
        run: Option<String>,
    },
    /// run をリセットする（要 --yes）。
    Reset {
        #[arg(long)]
        run: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// run を放棄する（terminal）。
    Abandon {
        run_id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// workflow.toml / spec.toml の静的検証。
    Validate {
        #[arg(long)]
        workflow: Option<String>,
        #[arg(long)]
        spec: Option<String>,
    },
    /// 現ノードの skill ファイルの絶対パスを表示する。
    Skill {
        #[arg(long)]
        run: Option<String>,
    },
    /// 現ノードの出口 gate を各 pass/fail で一覧する。
    Gates {
        #[arg(long)]
        run: Option<String>,
    },
    /// runtime ループを駆動する。`--script` あればスクリプト worker、無ければ生 API の ApiWorker。
    Run {
        /// スクリプト TOML パス。指定があれば ScriptedWorker、無ければ ApiWorker（ANTHROPIC_API_KEY 必須）。
        #[arg(long)]
        script: Option<String>,
        #[arg(long)]
        run: Option<String>,
        /// 作業ディレクトリ（worktree モード ── 編集/コマンドはこの cwd 基準。skeleton では隔離は scaffold）。
        #[arg(long)]
        worktree: Option<String>,
        /// ApiWorker のモデル override。`workflow.toml` の `[meta].default_model` を上書きする（任意）。
        #[arg(long)]
        model: Option<String>,
    },
    /// ノードごとの metrics（tool_calls / wall_seconds / cost / tokens）を表示する。
    Stats { run_id: String },
    /// 既存 repo に `.harness/` をスキャフォールド（プロジェクト検出＋スモークチェック）。
    Init { dir: Option<String>, #[arg(long)] force: bool },
    /// `.harness/` の健全性チェック（validate + gate cmd + skill ファイル）。
    Doctor { dir: Option<String>, #[arg(long)] full: bool },
    /// 指定ファイルの outline（トップレベル/主要シンボル）を表示する。CKG layer 1。
    Outline { path: String, #[arg(long, default_value = "text")] format: String },
    /// workspace のシンボル検索。CKG layer 2 (多言語 LSP)。
    FindSymbol { query: String, #[arg(long)] kind: Option<String>, #[arg(long)] root: Option<String>, #[arg(long, default_value = "text")] format: String, #[arg(long, default_value = "auto")] lang: String },
    /// 指定 symbol への参照箇所一覧。CKG layer 2 (多言語 LSP)。
    Refs { qname: String, #[arg(long)] root: Option<String>, #[arg(long, default_value = "text")] format: String, #[arg(long, default_value = "auto")] lang: String },
    /// 指定 function の呼び出し元一覧。CKG layer 2 (多言語 LSP)。
    Callers { qname: String, #[arg(long)] root: Option<String>, #[arg(long, default_value = "text")] format: String, #[arg(long, default_value = "auto")] lang: String },
    /// refs/callers の transitive 閉包。CKG layer 2 (多言語 LSP)。
    Closure { qname: String, #[arg(long, default_value_t = 2)] depth: usize, #[arg(long, default_value = "in")] direction: String, #[arg(long)] root: Option<String>, #[arg(long, default_value = "text")] format: String, #[arg(long, default_value = "auto")] lang: String },
    /// 変更影響範囲評価。closure direction=in の薄いラッパ。CKG layer 2 (多言語 LSP)。
    ImpactedBy { qname: String, #[arg(long, default_value_t = 3)] depth: usize, #[arg(long)] root: Option<String>, #[arg(long, default_value = "text")] format: String, #[arg(long, default_value = "auto")] lang: String },
    /// 指定 symbol をテストしている test 関数一覧。CKG layer 2 (多言語 LSP)。
    TestedBy { qname: String, #[arg(long, default_value_t = 3)] depth: usize, #[arg(long)] root: Option<String>, #[arg(long, default_value = "text")] format: String, #[arg(long, default_value = "auto")] lang: String },
    /// CKG layer 2 の query primitive ファサード。
    Query { #[command(subcommand)] cmd: crate::cli_query::QueryCmd },
}

/// CLI エントリポイント。`main.rs` から呼ばれる。
pub fn run() -> Result<(), String> {
    let cli = Cli::parse();
    dispatch(cli.command)
}
