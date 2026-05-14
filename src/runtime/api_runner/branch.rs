//! ApiWorker 経路の fork branch ── 1 branch を生 LLM API で駆動する足場（Phase 2）。
//!
//! scripted 版（runtime::fork_join_branch::branch_thread）の API 版相当。
//! 差分は次の 2 点:
//! - worker は ApiWorker::drive 経由で動く（tool_use ループは ApiWorker が回す）。
//! - apply_fn は apply_dispatch::apply_one ではなく branch 用 ── artifact / evidence を
//!   main run の jsonl ではなく branch sub-log (<run-id>.<branch_id>.jsonl) に書く。
//!
//! 今回は足場 ── 配線（並列 spawn / fold-in）は別タスク。run_loop / fork_join から
//! は呼ばれない。dead_code allow を付けてビルドだけ通す。
//!
//! ## なぜ apply_fn を分けたか
//! scripted 版 branch_thread は branch の artifact/evidence を append_branch_event で
//! sub-log に書く。これを main run の apply_action 経路に流すと、cmd_record_artifact が
//! main の jsonl に直接書いてしまい、join 時の fold-in（branch ログ → main 状態へ畳む）
//! 設計が崩れる。ApiWorker 経路でも同じ不変条件を保つため、apply_fn だけ branch 用に
//! 差し替える。
//!
//! ## RequestTransition / Back の扱い
//! branch 内では main 状態を進めない。branch_thread 同様、request_transition を受けたら
//! この branch を done として早期 return する。back / stuck は branch 失敗として
//! Err を返し、join 側で failures に積まれる前提。
//!
//! ファイル分割:
//! - branch.rs ── drive_branch_api 本体（このファイル）
//! - branch_apply.rs ── apply_fn ヘルパー（artifact / evidence / file write の sub-log 版）



use std::path::Path;
use std::time::Instant;

use crate::event::{append_branch_event, read_events, EventKind};
use crate::handlers::state_for;
use crate::handlers2::RunCtx;
use crate::metrics::{append_metrics, NodeMetrics, TokenBreakdown};
use crate::runtime::api_worker::{ApiWorker, ApplyResult, Outcome};
use crate::runtime::auth::AuthMode;
use crate::runtime::context;
use crate::runtime::http_client::HttpClient;
use crate::runtime::interceptor::Interceptor;
use crate::runtime::tools::ToolCall;
use crate::workflow::{Budget, Node, Workflow};

use super::branch_apply::branch_apply_one;

/// ApiWorker 経路で 1 branch を駆動する（足場）。
///
/// 配線:
/// - branch ノードを wf から検索（未定義なら Err）。
/// - WorkerContext は branch ノードを現ノードとして組む（scripted 版と context.rs 共通）。
/// - branch 内ツール呼び出しは branch_apply_one を通す（sub-log に書く）。
/// - ApiWorker::drive の Outcome を branch の done/fail にマップする。
pub fn drive_branch_api(
    run_id: &str,
    branch_id: &str,
    wf: &Workflow,
    http: &dyn HttpClient,
    auth: &AuthMode,
    model_default: &str,
    cwd: &Path,
) -> Result<(), String> {
    let node: Node = wf
        .nodes()
        .iter()
        .find(|n| n.id == branch_id)
        .cloned()
        .ok_or_else(|| format!("branch '{branch_id}' は workflow.toml に未定義"))?;

    let st = state_for(run_id, wf)?;
    let rc = RunCtx::load(run_id);
    let events = read_events(run_id)?;
    let ctx = context::build_context(wf, &node, &st, &rc, &events);

    let model = node.model.clone().unwrap_or_else(|| model_default.to_string());
    let budget: Budget = node.budget.clone().unwrap_or_else(|| {
        wf.meta.default_budget.clone().unwrap_or_default()
    });

    let intc = Interceptor::for_node(&node, rc.spec.as_ref(), cwd.to_path_buf());
    let mut tool_calls_for_budget: u64 = 0;

    let run_id_owned = run_id.to_string();
    let branch_id_owned = branch_id.to_string();
    let mut apply_fn = |call: ToolCall| -> ApplyResult {
        branch_apply_one(
            &run_id_owned,
            &branch_id_owned,
            &intc,
            call,
            &mut tool_calls_for_budget,
        )
    };

    let worker = ApiWorker::new(auth.clone(), model, http);
    let node_start = Instant::now();
    let (outcome, metrics) = worker.drive(&ctx, &budget, &mut apply_fn);
    let elapsed = node_start.elapsed().as_secs_f64();

    // branch ごとに main の metrics サイドカーへ 1 行（main path の spawn と同形式）。
    // `node` は branch_id ── join 側で集約しやすいよう main path と区別がつく。
    let breakdown = TokenBreakdown {
        input: metrics.usage.input_tokens,
        output: metrics.usage.output_tokens,
        cache_create: metrics.usage.cache_creation_input_tokens,
        cache_read: metrics.usage.cache_read_input_tokens,
    };
    let m = NodeMetrics::api(branch_id, metrics.tool_calls, elapsed, breakdown, metrics.cost_usd);
    append_metrics(run_id, &m)?;

    match outcome {
        Outcome::Transitioned => {
            append_branch_event(
                run_id,
                branch_id,
                EventKind::Advance {
                    from: branch_id.to_string(),
                    to: "(branch-done)".into(),
                },
            )
            .map_err(|e| format!("branch advance write fail: {e}"))?;
            Ok(())
        }
        Outcome::WentBack(reason) => Err(format!("back: {reason}")),
        Outcome::Stuck(reason) => Err(format!("stuck: {reason}")),
        Outcome::BudgetExceeded(reason) => Err(format!("budget: {reason}")),
        Outcome::ApiError(reason) => Err(format!("api_error: {reason}")),
        Outcome::NoToolUse => Err(format!(
            "no_tool_use: branch {branch_id} は tool_use 無しで end_turn"
        )),
    }
}
