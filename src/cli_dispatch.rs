//! `cli::run` の match dispatch ── 200 行制約のため cli.rs から切り出し。
//!
//! 各 Command バリアントを対応する handler 関数に振り分けるだけ。

use crate::cli::Command;
use crate::{
    handlers, handlers2, handlers3, handlers_advance, handlers_closure, handlers_find_symbol,
    handlers_init, handlers_outline, handlers_refs, handlers_stats, runtime,
};

/// `Cli::parse()` 後の Command を実行する。
pub fn dispatch(command: Command) -> Result<(), String> {
    match command {
        Command::Start { intent, worktree } => handlers::cmd_start(&intent, worktree.as_deref()),
        Command::Status { run } => handlers::cmd_status(run.as_deref()),
        Command::Advance { run, worktree: _ } => handlers_advance::cmd_advance(run.as_deref()),
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
        Command::Run { script, run, worktree, model } => match script {
            Some(s) => runtime::cmd_run(&s, run.as_deref(), worktree.as_deref()),
            None => runtime::cmd_run_api(run.as_deref(), worktree.as_deref(), model.as_deref()),
        },
        Command::Stats { run_id } => handlers_stats::cmd_stats(&run_id),
        Command::Init { dir, force } => handlers_init::cmd_init(dir.as_deref(), force),
        Command::Doctor { dir, full } => handlers_init::cmd_doctor(dir.as_deref(), full),
        Command::Outline { path, format } => handlers_outline::cmd_outline(&path, &format),
        Command::FindSymbol { query, kind, root, format } => {
            handlers_find_symbol::cmd_find_symbol(&query, kind.as_deref(), root.as_deref(), &format)
        }
        Command::Refs { qname, root, format } => {
            handlers_refs::cmd_refs(&qname, root.as_deref(), &format)
        }
        Command::Callers { qname, root, format } => {
            handlers_refs::cmd_callers(&qname, root.as_deref(), &format)
        }
        Command::Closure { qname, depth, direction, root, format } => {
            handlers_closure::cmd_closure(&qname, depth, &direction, root.as_deref(), &format)
        }
    }
}
