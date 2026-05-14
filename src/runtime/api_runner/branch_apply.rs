//! branch 内ツール呼び出しの apply ヘルパー（branch.rs のサポート）。
//!
//! main run 版 `apply_dispatch::apply_one` との差:
//! - artifact / evidence は `append_branch_event` で sub-log に書く（main の jsonl に
//!   直接書かない ── fold-in 設計を守る）。
//! - request_transition / back / stuck は terminal Outcome を返すだけで、main 状態の
//!   advance/back を起こさない（branch_thread の挙動と合わせる）。
//! - run_command / ask は branch 内では未対応（必要になれば後続タスクで足す）。

#![allow(dead_code)]

use crate::event::{append_branch_event, EventKind};
use crate::runtime::api_worker::{ApplyResult, Outcome};
use crate::runtime::interceptor::{Interceptor, Verdict};
use crate::runtime::tools::ToolCall;
use crate::runtime::worker::WorkerAction;

fn ok(content: String) -> ApplyResult {
    ApplyResult { content, is_error: false, terminal: None }
}
fn err(content: String) -> ApplyResult {
    ApplyResult { content, is_error: true, terminal: None }
}
fn terminal(content: String, t: Outcome) -> ApplyResult {
    ApplyResult { content, is_error: false, terminal: Some(t) }
}

/// branch 内 1 ツール呼び出しの apply（branch.rs からのみ呼ばれる）。
pub(super) fn branch_apply_one(
    run_id: &str,
    branch_id: &str,
    intc: &Interceptor,
    call: ToolCall,
    tool_calls_for_budget: &mut u64,
) -> ApplyResult {
    match call {
        ToolCall::ReadFile(path) => read_file_branch(intc, &path),
        ToolCall::Action(action) => {
            if !matches!(action, WorkerAction::Stuck { .. }) {
                *tool_calls_for_budget += 1;
            }
            apply_branch_action(run_id, branch_id, intc, action)
        }
    }
}

fn read_file_branch(intc: &Interceptor, path: &str) -> ApplyResult {
    let full = intc.cwd().join(path);
    match std::fs::read_to_string(&full) {
        Ok(text) if text.len() > 4000 => {
            ok(format!("{}\n…（{} 文字、頭 4000 のみ表示）", &text[..4000], text.len()))
        }
        Ok(text) => ok(text),
        Err(e) => err(format!("read_file 失敗 {}: {e}", full.display())),
    }
}

/// `apply_branch_action` ── テストから呼べるよう同クレート内のみ可視。
pub(super) fn apply_branch_action(
    run_id: &str,
    branch_id: &str,
    intc: &Interceptor,
    action: WorkerAction,
) -> ApplyResult {
    match action {
        WorkerAction::RecordArtifact { name, path } => {
            record_artifact_branch(run_id, branch_id, intc, &name, &path)
        }
        WorkerAction::ReportEvidence { gate, json } => {
            report_evidence_branch(run_id, branch_id, &gate, &json)
        }
        WorkerAction::CreateFile { path, content } => {
            write_file_branch(branch_id, intc, &path, &content, false)
        }
        WorkerAction::EditFile { path, content } => {
            edit_file_branch(branch_id, intc, &path, &content)
        }
        WorkerAction::RequestTransition => {
            terminal("request_transition: branch を done として返す".into(), Outcome::Transitioned)
        }
        WorkerAction::Back { reason } => {
            let r = reason.clone();
            terminal(format!("back: {reason}"), Outcome::WentBack(r))
        }
        WorkerAction::Stuck { reason } => {
            let r = reason.clone();
            terminal(format!("stuck: {reason}"), Outcome::Stuck(r))
        }
        WorkerAction::RunCommand { cmd } => err(format!("branch 内では run_command 未対応: {cmd}")),
        WorkerAction::Ask { question, .. } => err(format!("branch 内では ask 未対応: {question}")),
    }
}

fn edit_file_branch(branch_id: &str, intc: &Interceptor, path: &str, content: &str) -> ApplyResult {
    let full = intc.cwd().join(path);
    if let Verdict::Deny(why) = intc.check_write(&full) {
        return err(format!("edit_file 拒否（branch {branch_id}）: {why}"));
    }
    write_file_branch(branch_id, intc, path, content, true)
}

fn record_artifact_branch(
    run_id: &str,
    branch_id: &str,
    intc: &Interceptor,
    name: &str,
    path: &str,
) -> ApplyResult {
    let full = intc.cwd().join(path);
    let canon = match full.canonicalize() {
        Ok(p) => p,
        Err(_) => return err(format!("artifact missing: {path}")),
    };
    match append_branch_event(
        run_id,
        branch_id,
        EventKind::Artifact {
            name: name.to_string(),
            path: canon.to_string_lossy().to_string(),
            tag: None,
        },
    ) {
        Ok(()) => ok(format!("artifact '{name}' 登録（branch {branch_id}）")),
        Err(e) => err(format!("branch artifact write fail: {e}")),
    }
}

fn report_evidence_branch(run_id: &str, branch_id: &str, gate: &str, json: &str) -> ApplyResult {
    let data: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => return err(format!("evidence JSON parse fail: {e}")),
    };
    match append_branch_event(
        run_id,
        branch_id,
        EventKind::GateEvidence { gate: gate.to_string(), data },
    ) {
        Ok(()) => ok(format!("evidence '{gate}' 記録（branch {branch_id}）")),
        Err(e) => err(format!("branch evidence write fail: {e}")),
    }
}

fn write_file_branch(
    branch_id: &str,
    intc: &Interceptor,
    path: &str,
    content: &str,
    is_edit: bool,
) -> ApplyResult {
    let full = intc.cwd().join(path);
    if let Some(parent) = full.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return err(format!("mkdir 失敗 {}: {e}", parent.display()));
        }
    }
    let label = if is_edit { "edit_file" } else { "create_file" };
    match std::fs::write(&full, content) {
        Ok(()) => ok(format!("ファイル '{path}' を書いた（branch {branch_id}, {label}）")),
        Err(e) => err(format!("{label} 失敗 {}: {e}", full.display())),
    }
}

#[cfg(test)]
#[path = "branch_apply_tests.rs"]
mod tests;
