//! ツール呼び出し 1 件の apply ── `run_loop` から切り出した責務分離モジュール。
//!
//! 含むもの:
//! - `apply_one` ── 1 `ToolCall` を harness へ反映、`ApplyResult` を組む。
//! - `pre_check` ── EditFile / RunCommand の interceptor 事前判定。
//! - `action_ok_str` ── 成功時の短い文言ビルダ。
//! - `effective_budget` ── ノード固有 budget か meta 既定の選択。
//! - `counts_as_tool_call` ── stuck を budget の tool_calls から除外する判定。

use crate::runtime::api_worker::{ApplyResult, Outcome};
use crate::runtime::apply::{apply_action, Applied};
use crate::runtime::interceptor::{Interceptor, Verdict};
use crate::runtime::tools::ToolCall;
use crate::runtime::worker::WorkerAction;
use crate::workflow::{Budget, Workflow};

/// `WorkerAction` のうち budget の「ツール呼び出し」としてカウントするもの（stuck は除外）。
pub(super) fn counts_as_tool_call(a: &WorkerAction) -> bool {
    !matches!(a, WorkerAction::Stuck { .. })
}

/// 1 ツール呼び出しを apply し、`ApplyResult` を返す。
pub(super) fn apply_one(
    run_id: &str,
    intc: &Interceptor,
    call: ToolCall,
    tool_calls_for_budget: &mut u64,
) -> ApplyResult {
    match call {
        ToolCall::ReadFile(path) => {
            let full = match intc.safe_resolve(&path) {
                Ok(p) => p,
                Err(why) => {
                    return ApplyResult {
                        content: format!("read_file 拒否: {why}"),
                        is_error: true,
                        terminal: None,
                    }
                }
            };
            match std::fs::read_to_string(&full) {
                Ok(text) => ApplyResult {
                    content: if text.len() > 4000 {
                        format!("{}\n…（{} 文字、頭 4000 のみ表示）", &text[..4000], text.len())
                    } else {
                        text
                    },
                    is_error: false,
                    terminal: None,
                },
                Err(e) => ApplyResult {
                    content: format!("read_file 失敗 {}: {e}", full.display()),
                    is_error: true,
                    terminal: None,
                },
            }
        }
        ToolCall::Action(action) => {
            if counts_as_tool_call(&action) {
                *tool_calls_for_budget += 1;
            }
            // 事前 interceptor チェック（blast radius / cmd_allowlist）── 失敗なら apply せず error 返す。
            if let Some(why) = pre_check(intc, &action) {
                return ApplyResult { content: why, is_error: true, terminal: None };
            }
            match apply_action(run_id, &action, intc) {
                Ok(Applied::Continued) => ApplyResult {
                    // edit_file 成功時はコスト0 security scan を回し、warning があれば追記する
                    // （非ブロッキング ── 書込は済んでいる、worker への注意喚起のみ）。
                    content: append_security_warning(action_ok_str(&action), &action),
                    is_error: false,
                    terminal: None,
                },
                Ok(Applied::Transitioned) => ApplyResult {
                    content: "request_transition: 評価完了 ── 次の status を確認しろ".to_string(),
                    is_error: false,
                    terminal: Some(Outcome::Transitioned),
                },
                Ok(Applied::WentBack) => ApplyResult {
                    content: "back: 前ノードへ戻った".to_string(),
                    is_error: false,
                    terminal: Some(Outcome::WentBack(match &action {
                        WorkerAction::Back { reason } => reason.clone(),
                        _ => String::new(),
                    })),
                },
                Ok(Applied::Stuck(reason)) => ApplyResult {
                    content: format!("stuck: 質問キューに積んだ ── {reason}"),
                    is_error: false,
                    terminal: Some(Outcome::Stuck(reason)),
                },
                Err(e) => ApplyResult {
                    content: format!("apply 失敗: {e}"),
                    is_error: true,
                    terminal: None,
                },
            }
        }
    }
}

fn pre_check(intc: &Interceptor, action: &WorkerAction) -> Option<String> {
    match action {
        WorkerAction::EditFile { path, .. } => {
            // まず cwd 配下へ安全解決（path traversal 拒否）、その後 blast radius 判定。
            let full = match intc.safe_resolve(path) {
                Ok(p) => p,
                Err(why) => return Some(format!("edit_file 拒否: {why}")),
            };
            match intc.check_write(&full) {
                Verdict::Allow => None,
                Verdict::Deny(why) => Some(format!("edit_file 拒否: {why}")),
            }
        }
        WorkerAction::RunCommand { cmd } => match intc.check_command(cmd) {
            Verdict::Allow => None,
            Verdict::Deny(why) => Some(format!("run_command 拒否: {why}")),
        },
        _ => None,
    }
}

/// edit_file / create_file の content をコスト0 security scan にかけ、
/// findings があれば成功メッセージに warning を追記する（非ブロッキング）。
fn append_security_warning(ok: String, action: &WorkerAction) -> String {
    use crate::runtime::security_scan::{format_warning, scan};
    let (path, content) = match action {
        WorkerAction::EditFile { path, content } => (path, content),
        WorkerAction::CreateFile { path, content } => (path, content),
        _ => return ok,
    };
    match format_warning(path, &scan(path, content)) {
        Some(w) => format!("{ok}\n{w}"),
        None => ok,
    }
}

fn action_ok_str(action: &WorkerAction) -> String {
    match action {
        WorkerAction::RecordArtifact { name, .. } => format!("artifact '{name}' 登録"),
        WorkerAction::ReportEvidence { gate, .. } => format!("evidence '{gate}' 記録"),
        WorkerAction::EditFile { path, .. } => format!("ファイル '{path}' を書いた"),
        WorkerAction::RunCommand { cmd } => format!("command '{cmd}' 実行"),
        WorkerAction::Ask { question, .. } => format!("質問を積んだ: {question}"),
        WorkerAction::Back { reason } => format!("back: {reason}"),
        _ => "ok".to_string(),
    }
}

pub(super) fn effective_budget(wf: &Workflow, node: &crate::workflow::Node) -> Budget {
    if let Some(b) = &node.budget {
        return b.clone();
    }
    wf.meta.default_budget.clone().unwrap_or_default()
}
