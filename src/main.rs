#![forbid(unsafe_code)]
//! 軽量ワークフローハーネス: CLI 定義と dispatch のみ。

mod commands;
mod gates;
mod paths;
mod phases;
mod state;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "harness", about = "軽量ワークフローハーネス (thin harness / fat skills / 決定論的状態管理)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 新しい run を開始する
    Start { intent: String },
    /// 現在の状態と出口 gate を表示
    Status {
        #[arg(long)]
        run: Option<String>,
    },
    /// 出口 gate を満たしていれば次フェーズへ
    Advance {
        #[arg(long)]
        run: Option<String>,
    },
    /// 前フェーズへ戻る
    Back {
        reason: String,
        #[arg(long)]
        run: Option<String>,
    },
    /// 成果物ファイルを登録（name 例: research_notes / plan / impl:<id>）
    RecordArtifact {
        name: String,
        path: String,
        #[arg(long)]
        run: Option<String>,
    },
    /// gate 根拠 (JSON) を報告（@file でファイル読込）
    ReportGate {
        gate: String,
        json: String,
        #[arg(long)]
        run: Option<String>,
    },
    /// この run を初期フェーズへリセット（--yes 必須）
    Reset {
        #[arg(long)]
        run: Option<String>,
        #[arg(long)]
        yes: bool,
    },
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Start { intent } => commands::cmd_start(intent),
        Command::Status { run } => commands::cmd_status(run),
        Command::Advance { run } => commands::cmd_advance(run),
        Command::Back { reason, run } => commands::cmd_back(reason, run),
        Command::RecordArtifact { name, path, run } => commands::cmd_record_artifact(name, path, run),
        Command::ReportGate { gate, json, run } => commands::cmd_report_gate(gate, json, run),
        Command::Reset { run, yes } => commands::cmd_reset(run, yes),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
