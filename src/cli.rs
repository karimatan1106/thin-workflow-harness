//! debug CLI ── clap derive とサブコマンド dispatch。
//!
//! 「Claude Code が `harness` コマンドを叩く」前提。runtime 層（worker spawn 等）は Phase 1。

use clap::{Parser, Subcommand};

use crate::{handlers, handlers2};

#[derive(Parser)]
#[command(name = "harness", about = "thin workflow harness (Phase 0 walking skeleton)")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// 新 run を開始する。
    Start { intent: String },
    /// run の状態を表示する。
    Status {
        #[arg(long)]
        run: Option<String>,
    },
    /// 現ノードの出口 gate を全評価し、全 pass なら次ノードへ進む。
    Advance {
        #[arg(long)]
        run: Option<String>,
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
    /// run をリセットする（要 --yes）。
    Reset {
        #[arg(long)]
        run: Option<String>,
        #[arg(long)]
        yes: bool,
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
}

/// CLI エントリポイント。`main.rs` から呼ばれる。
pub fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Start { intent } => handlers::cmd_start(&intent),
        Command::Status { run } => handlers::cmd_status(run.as_deref()),
        Command::Advance { run } => handlers::cmd_advance(run.as_deref()),
        Command::Back { reason, run } => handlers::cmd_back(&reason, run.as_deref()),
        Command::RecordArtifact { name, path, tag, run } => {
            handlers::cmd_record_artifact(&name, &path, tag.as_deref(), run.as_deref())
        }
        Command::ReportEvidence { gate, json, run } => {
            handlers::cmd_report_evidence(&gate, &json, run.as_deref())
        }
        Command::Reset { run, yes } => handlers::cmd_reset(run.as_deref(), yes),
        Command::Validate { workflow, spec } => {
            handlers2::cmd_validate(workflow.as_deref(), spec.as_deref())
        }
        Command::Skill { run } => handlers2::cmd_skill(run.as_deref()),
        Command::Gates { run } => handlers2::cmd_gates(run.as_deref()),
    }
}
