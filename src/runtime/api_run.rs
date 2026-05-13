//! `harness run [--model M]` ── `ApiWorker` で runtime ループを駆動する経路。
//!
//! `runtime/mod.rs` の `cmd_run`（ScriptedWorker）と並行する関数。両者の差はワーカー実装と
//! apply ループの形だけ ── ScriptedWorker は actions を順に取って runtime が apply、ApiWorker は
//! 内部で tool-use ループを回して 1 ツールずつ apply（結果文字列を会話に戻す必要があるため）。

use std::time::Instant;

use crate::event::{append_event, read_events, EventKind};
use crate::handlers::{load_wf, state_for};
use crate::handlers2::RunCtx;
use crate::metrics::{append_metrics, NodeMetrics, TokenBreakdown};
use crate::paths;
use crate::runtime::api_worker::{ApiWorker, ApplyResult, Outcome};
use crate::runtime::apply::{apply_action, has_pending_escalation, rejected_since_transition, Applied};
use crate::runtime::auth::{resolve_auth, AuthMode};
use crate::runtime::context;
use crate::runtime::http_client::{HttpClient, UreqClient};
use crate::runtime::interceptor::{Interceptor, Verdict};
use crate::runtime::tools::ToolCall;
use crate::runtime::worker::WorkerAction;
use crate::workflow::{current_node, Budget, Workflow};

/// 暴走防止 ── ノード spawn の最大回数。
const MAX_SPAWNS: usize = 256;
/// `WorkerAction` のうち budget の「ツール呼び出し」としてカウントするもの（stuck は除外）。
fn counts_as_tool_call(a: &WorkerAction) -> bool {
    !matches!(a, WorkerAction::Stuck { .. })
}

/// `--script` 無しで `harness run` が呼ばれたときのエントリポイント。
pub fn cmd_run_api(
    run: Option<&str>,
    worktree: Option<&str>,
    model_override: Option<&str>,
) -> Result<(), String> {
    let http = UreqClient::default();
    cmd_run_api_with(run, worktree, model_override, &http)
}

/// `cmd_run_api` の HTTP クライアント注入版 ── テストで `MockClient` を渡すために使う。
pub fn cmd_run_api_with(
    run: Option<&str>,
    worktree: Option<&str>,
    model_override: Option<&str>,
    http: &dyn HttpClient,
) -> Result<(), String> {
    let auth = resolve_auth()?;
    cmd_run_api_with_auth(run, worktree, model_override, http, auth)
}

/// 認証モードも注入する版 ── テストで `AuthMode` を直に渡す。
pub fn cmd_run_api_with_auth(
    run: Option<&str>,
    worktree: Option<&str>,
    model_override: Option<&str>,
    http: &dyn HttpClient,
    auth: AuthMode,
) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let wf = load_wf()?;
    let cwd = worktree
        .map(std::path::PathBuf::from)
        .unwrap_or_else(paths::harness_home);
    let model_default = default_model(&wf, model_override);
    println!(
        "[runtime] run {run_id} を ApiWorker で駆動 cwd={} model_default={model_default}",
        cwd.display()
    );

    for spawn_n in 1..=MAX_SPAWNS {
        let st = state_for(&run_id, &wf)?;
        if st.abandoned {
            return Err(format!("run {run_id} は放棄済み ── runtime は駆動できない"));
        }
        if st.done {
            println!("[runtime] 全ノード完了 ── status: done");
            return Ok(());
        }
        if has_pending_escalation(&run_id) {
            return Err(format!(
                "run {run_id} は人間の判断待ち（escalation 質問あり、`harness questions` 参照）"
            ));
        }
        let Some(node) = current_node(&wf, &st) else {
            println!("[runtime] 全ノード完了 ── status: done");
            return Ok(());
        };
        let node = node.clone();
        let model = node.model.clone().unwrap_or_else(|| model_default.clone());
        let rc = RunCtx::load(&run_id);
        let events = read_events(&run_id)?;
        let ctx = context::build_context(&wf, &node, &st, &rc, &events);
        println!(
            "[node {}] ApiWorker spawn (#{spawn_n}) model={model} tools={}",
            node.id,
            ctx.tools.join(" ")
        );
        let budget = effective_budget(&wf, &node);
        let intc = Interceptor::for_node(&node, rc.spec.as_ref(), cwd.clone());
        let mut tool_calls_for_budget: u64 = 0;
        let mut apply_fn = |call: ToolCall| -> ApplyResult {
            apply_one(&run_id, &intc, call, &mut tool_calls_for_budget)
        };

        let worker = ApiWorker::new(auth.clone(), model, http);
        let node_start = Instant::now();
        let (outcome, metrics) = worker.drive(&ctx, &budget, &mut apply_fn);
        let elapsed = node_start.elapsed().as_secs_f64();

        match &outcome {
            Outcome::Stuck(_) => {
                return Err(format!(
                    "run {run_id}: worker が詰まったと申告 ── 人間の判断待ち（`harness questions`）"
                ));
            }
            Outcome::BudgetExceeded(reason) | Outcome::ApiError(reason) => {
                let kind = if matches!(outcome, Outcome::BudgetExceeded(_)) {
                    "budget"
                } else {
                    "api_error"
                };
                append_event(
                    &run_id,
                    EventKind::AdvanceRejected {
                        failed_gates: vec![crate::event::FailedGate {
                            gate: format!("node_aborted:{kind}"),
                            reason: reason.clone(),
                        }],
                    },
                )
                .map_err(|e| format!("advance_rejected 書込失敗: {e}"))?;
            }
            _ => {}
        }

        // ノード境界で metrics を 1 行 ── ApiWorker は tokens/cost 付き。
        let breakdown = TokenBreakdown {
            input: metrics.usage.input_tokens,
            output: metrics.usage.output_tokens,
            cache_create: metrics.usage.cache_creation_input_tokens,
            cache_read: metrics.usage.cache_read_input_tokens,
        };
        let m = NodeMetrics::api(&node.id, metrics.tool_calls, elapsed, breakdown, metrics.cost_usd);
        append_metrics(&run_id, &m)?;

        // 状態 delta を見て、次の spawn / 完了 / 再 spawn を決める。
        let after = state_for(&run_id, &wf)?;
        let ev_after = read_events(&run_id)?;
        if after.phase_index > st.phase_index {
            println!(
                "[node {}] → advance（次ノードへ） api_calls={} tool_calls={} wall={:.3}s",
                node.id, metrics.api_calls, metrics.tool_calls, elapsed
            );
        } else if after.phase_index < st.phase_index {
            println!("[node {}] → back", node.id);
        } else if rejected_since_transition(&ev_after) {
            println!("[node {}] → advance_rejected ── fresh で再 spawn する", node.id);
        } else {
            println!("[node {}] outcome={:?} ── 遷移なし、次 spawn で再評価", node.id, outcome);
        }
    }
    Err(format!("run spawn 上限 {MAX_SPAWNS} に到達"))
}

/// 1 ツール呼び出しを apply し、`ApplyResult` を返す。
fn apply_one(
    run_id: &str,
    intc: &Interceptor,
    call: ToolCall,
    tool_calls_for_budget: &mut u64,
) -> ApplyResult {
    match call {
        ToolCall::ReadFile(path) => {
            let full = intc.cwd().join(&path);
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
                    content: action_ok_str(&action),
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
            let full = intc.cwd().join(path);
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

fn effective_budget(wf: &Workflow, node: &crate::workflow::Node) -> Budget {
    if let Some(b) = &node.budget {
        return b.clone();
    }
    wf.meta.default_budget.clone().unwrap_or_default()
}

fn default_model(wf: &Workflow, override_: Option<&str>) -> String {
    if let Some(m) = override_ {
        return m.to_string();
    }
    wf.meta
        .default_model
        .clone()
        .unwrap_or_else(|| "claude-opus-4-7".to_string())
}

