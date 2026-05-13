//! fork_join のサポート: branch 内 thread 本体 と action ラベル整形。
//!
//! 元 fork_join.rs を 200 行ルールに収めるために分離。

use serde_json::Value as JsonValue;

use crate::event::{append_branch_event, EventKind};
use crate::runtime::script::Step;
use crate::runtime::worker::WorkerAction;

pub(super) fn branch_thread(run_id: &str, branch_id: &str, steps: Vec<Step>) -> Result<(), String> {
    if steps.is_empty() {
        return Err(format!("branch {branch_id} has no matching step (script mismatch)"));
    }
    for step in steps {
        for act in step.actions {
            println!("[branch {branch_id}] action: {}", action_label(&act));
            match act {
                WorkerAction::RecordArtifact { name, path } => {
                    let cwd = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
                    let canon = cwd
                        .join(&path)
                        .canonicalize()
                        .map_err(|_| format!("[branch {branch_id}] artifact missing: {path}"))?;
                    append_branch_event(
                        run_id,
                        branch_id,
                        EventKind::Artifact {
                            name,
                            path: canon.to_string_lossy().to_string(),
                            tag: None,
                        },
                    )
                    .map_err(|e| format!("branch artifact write fail: {e}"))?;
                }
                WorkerAction::ReportEvidence { gate, json } => {
                    let data: JsonValue = serde_json::from_str(&json)
                        .map_err(|e| format!("[branch {branch_id}] evidence JSON: {e}"))?;
                    append_branch_event(run_id, branch_id, EventKind::GateEvidence { gate, data })
                        .map_err(|e| format!("branch gate_evidence write fail: {e}"))?;
                }
                WorkerAction::CreateFile { path, content } => {
                    let cwd = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
                    let full = cwd.join(&path);
                    if let Some(parent) = full.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("mkdir: {e}"))?;
                    }
                    std::fs::write(&full, &content)
                        .map_err(|e| format!("[branch {branch_id}] write {path}: {e}"))?;
                }
                WorkerAction::RequestTransition => {
                    append_branch_event(
                        run_id,
                        branch_id,
                        EventKind::Advance { from: branch_id.to_string(), to: "(branch-done)".into() },
                    )
                    .map_err(|e| format!("branch advance write fail: {e}"))?;
                    return Ok(());
                }
                WorkerAction::Stuck { reason } => return Err(format!("stuck: {reason}")),
                WorkerAction::Back { reason } => return Err(format!("back: {reason}")),
                other => return Err(format!("unsupported in branch: {}", action_label(&other))),
            }
        }
    }
    Err(format!("branch {branch_id} exhausted steps without request_transition"))
}

fn action_label(a: &WorkerAction) -> String {
    match a {
        WorkerAction::CreateFile { path, .. } => format!("create_file({path})"),
        WorkerAction::EditFile { path, .. } => format!("edit_file({path})"),
        WorkerAction::RunCommand { cmd } => format!("run_command({cmd})"),
        WorkerAction::RecordArtifact { name, .. } => format!("record_artifact({name})"),
        WorkerAction::ReportEvidence { gate, .. } => format!("report_evidence({gate})"),
        WorkerAction::RequestTransition => "request_transition".into(),
        WorkerAction::Back { .. } => "back".into(),
        WorkerAction::Ask { .. } => "ask".into(),
        WorkerAction::Stuck { .. } => "stuck".into(),
    }
}
