//! debug CLI ── clap derive とサブコマンド dispatch。
//!
//! 「Claude Code が `harness` コマンドを叩く」前提。runtime 層（worker spawn 等）は Phase 1。

use clap::{Parser, Subcommand};

use crate::{handlers, handlers2, handlers3, handlers_advance};

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
}

/// CLI エントリポイント。`main.rs` から呼ばれる。
pub fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Start { intent } => handlers::cmd_start(&intent),
        Command::Status { run } => handlers::cmd_status(run.as_deref()),
        Command::Advance { run } => handlers_advance::cmd_advance(run.as_deref()),
        Command::Back { reason, run } => handlers::cmd_back(&reason, run.as_deref()),
        Command::RecordArtifact { name, path, tag, run } => {
            handlers::cmd_record_artifact(&name, &path, tag.as_deref(), run.as_deref())
        }
        Command::ReportEvidence { gate, json, run } => {
            handlers::cmd_report_evidence(&gate, &json, run.as_deref())
        }
        Command::Ask { question, option, header, kind, required, run } => handlers3::cmd_ask(
            &question,
            &option,
            header.as_deref(),
            kind.as_deref(),
            required,
            run.as_deref(),
        ),
        Command::Questions { run } => handlers3::cmd_questions(run.as_deref()),
        Command::Answer { question_id, choice, run } => {
            handlers3::cmd_answer(&question_id, &choice, run.as_deref())
        }
        Command::Reset { run, yes } => handlers::cmd_reset(run.as_deref(), yes),
        Command::Abandon { run_id, reason } => handlers3::cmd_abandon(&run_id, reason.as_deref()),
        Command::Validate { workflow, spec } => {
            handlers2::cmd_validate(workflow.as_deref(), spec.as_deref())
        }
        Command::Skill { run } => handlers2::cmd_skill(run.as_deref()),
        Command::Gates { run } => handlers2::cmd_gates(run.as_deref()),
    }
}
