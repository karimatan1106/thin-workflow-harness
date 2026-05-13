//! runtime 層（Phase 1 skeleton）── ノード間ループとスクリプト worker による駆動。
//!
//! ループ本体: state ロード → 現ノード → context バンドル構築 → worker → アクション適用
//! （既存 advance 経路で gate 評価）→ advance / reject → fresh 再 spawn → 次ノード → done/abandoned。
//! ノード完了（advance）時に metrics サイドカー（`state/<run-id>.metrics.jsonl`）へ 1 行追記する。
//!
//! 生 Anthropic API クライアント + tool-use ループは Step 4 以降。ここではスクリプト worker
//! （決定論的スタンドイン）でループを end-to-end テスト可能にし、オーケストレーションを de-risk する。

pub mod anthropic;
pub mod api_run;
pub mod api_runner;
pub mod api_worker;
pub mod apply;
pub mod auth;
pub mod auth_credentials;
pub mod context;
pub mod http_client;
pub mod interceptor;
pub mod script;
pub mod tool_schemas;
pub mod tools;
pub mod worker;

pub use api_run::{cmd_run_api, cmd_run_api_with};

use std::time::Instant;

use crate::event::read_events;
use crate::handlers::{load_wf, state_for};
use crate::handlers2::RunCtx;
use crate::metrics::{append_metrics, NodeMetrics};
use crate::paths;
use crate::runtime::apply::{apply_action, has_pending_escalation, rejected_since_transition, Applied};
use crate::runtime::interceptor::Interceptor;
use crate::runtime::worker::{ScriptedWorker, Worker, WorkerAction, WorkerContext};
use crate::workflow::current_node;

/// 暴走防止 ── ノード spawn の最大回数。
const MAX_SPAWNS: usize = 256;

/// 「ツール呼び出し」とカウントするアクションか（create_file / edit_file / run_command / record-artifact /
/// report-evidence / request-transition / back / ask）── stuck は除く。
fn counts_as_tool_call(a: &WorkerAction) -> bool {
    !matches!(a, WorkerAction::Stuck { .. })
}

/// `harness run --script <path> [--run R] [--worktree P]` ── runtime ループをスクリプト worker で駆動する。
pub fn cmd_run(script_path: &str, run: Option<&str>, worktree: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let wf = load_wf()?;
    let steps = script::load_script(std::path::Path::new(script_path))?;
    let worker = ScriptedWorker::new(steps);
    let cwd = worktree
        .map(std::path::PathBuf::from)
        .unwrap_or_else(paths::harness_home);
    println!("[runtime] run {run_id} をスクリプト worker で駆動 ({script_path})  cwd={}", cwd.display());

    // 同一ノードへの連続 spawn にまたがって tool_calls / 経過時間を積む。
    let mut node_tool_calls: u64 = 0;
    let mut node_start = Instant::now();
    let mut last_node_id: Option<String> = None;

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
        if last_node_id.as_deref() != Some(node.id.as_str()) {
            node_tool_calls = 0;
            node_start = Instant::now();
            last_node_id = Some(node.id.clone());
        }

        let rc = RunCtx::load(&run_id);
        let events = read_events(&run_id)?;
        let ctx = context::build_context(&wf, &node, &st, &rc, &events);
        log_bundle(spawn_n, &node.id, &ctx);

        let intc = Interceptor::for_node(&node, rc.spec.as_ref(), cwd.clone());
        let actions = worker.act(&ctx);
        println!("[node {}] worker のアクション [{}]", node.id, summarize_actions(&actions));

        let mut stuck_reason: Option<String> = None;
        for act in &actions {
            if counts_as_tool_call(act) {
                node_tool_calls += 1;
            }
            match apply_action(&run_id, act, &intc)? {
                Applied::Continued => {}
                Applied::Transitioned | Applied::WentBack => break,
                Applied::Stuck(reason) => {
                    stuck_reason = Some(reason);
                    break;
                }
            }
        }
        if let Some(reason) = stuck_reason {
            return Err(format!(
                "run {run_id}: worker が詰まったと申告（{reason}）── 人間の判断待ち（`harness questions`）"
            ));
        }

        // 遷移結果を state delta で判定する。
        let after = state_for(&run_id, &wf)?;
        let ev_after = read_events(&run_id)?;
        if after.phase_index > st.phase_index {
            // ノード完了 ── metrics サイドカーへ 1 行。
            let m = NodeMetrics::scripted(&node.id, node_tool_calls, node_start.elapsed().as_secs_f64());
            append_metrics(&run_id, &m)?;
            println!(
                "[node {}] → advance（次ノードへ） metrics: tool_calls={} wall={:.3}s",
                node.id, node_tool_calls, m.wall_seconds
            );
            last_node_id = None; // 次ノードでリセット
        } else if after.phase_index < st.phase_index {
            println!("[node {}] → back", node.id);
            last_node_id = None;
        } else if rejected_since_transition(&ev_after) {
            println!("[node {}] → advance_rejected ── fresh で再 spawn する", node.id);
        } else {
            println!("[node {}] 遷移なし ── 次 spawn で再評価", node.id);
        }
    }
    Err(format!("run {run_id}: spawn 回数が上限 {MAX_SPAWNS} に達した ── スクリプトが収束しない"))
}

fn log_bundle(spawn_n: usize, node_id: &str, ctx: &WorkerContext) {
    let respawn = if ctx.is_respawn() { " (再 spawn)" } else { "" };
    println!("[node {node_id}] context バンドルを worker に渡す{respawn} [spawn #{spawn_n}]");
    println!("  tools: {}", ctx.tools.join(" "));
    if !ctx.spec_slice.is_empty() {
        let first = ctx.spec_slice.lines().next().unwrap_or("");
        println!("  spec: {first}");
    }
    if ctx.is_respawn() {
        let names: Vec<&str> = ctx.failed_gates.iter().map(|(g, _)| g.as_str()).collect();
        println!("  直前 failed_gates: {}", names.join(", "));
    }
}

fn summarize_actions(actions: &[worker::WorkerAction]) -> String {
    use worker::WorkerAction as A;
    actions
        .iter()
        .map(|a| match a {
            A::CreateFile { path, .. } => format!("create_file({path})"),
            A::EditFile { path, .. } => format!("edit_file({path})"),
            A::RunCommand { cmd } => format!("run_command({cmd})"),
            A::RecordArtifact { name, .. } => format!("record_artifact({name})"),
            A::ReportEvidence { gate, .. } => format!("report_evidence({gate})"),
            A::RequestTransition => "request_transition".to_string(),
            A::Back { .. } => "back".to_string(),
            A::Ask { .. } => "ask".to_string(),
            A::Stuck { .. } => "stuck".to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
