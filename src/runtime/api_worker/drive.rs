//! drive ループ本体 ── `ApiWorker::drive` の中身。
//!
//! message 構築（system / initial user）は `system.rs` に分離。
//! ここはターン回し・budget チェック・usage 累積・finalize だけを担う。

use std::time::Instant;

use crate::runtime::anthropic::{estimate_cost_usd, Message, MessagesRequest, Usage};
use crate::runtime::auth::AuthMode;
use crate::runtime::http_client::HttpClient;
use crate::runtime::tools::{tool_defs, ToolCall};
use crate::runtime::worker::WorkerContext;
use crate::workflow::Budget;

use super::apply_loop::run_tool_uses;
use super::retry::call_with_retry;
use super::system::{build_system_blocks, initial_user_message};
use super::{ApiWorkerMetrics, ApplyResult, Outcome, DEFAULT_MAX_TOKENS, HARD_TURN_LIMIT};

/// drive ループの本体。`ApiWorker::drive` から呼ばれる薄い entry。
pub(super) fn drive(
    auth: &AuthMode,
    model: &str,
    http: &dyn HttpClient,
    ctx: &WorkerContext,
    budget: &Budget,
    apply_fn: &mut dyn FnMut(ToolCall) -> ApplyResult,
) -> (Outcome, ApiWorkerMetrics) {
    let start = Instant::now();
    let mut metrics = ApiWorkerMetrics::default();
    let mut messages: Vec<Message> = vec![initial_user_message(ctx)];
    let system_blocks = build_system_blocks(ctx);
    let tools = tool_defs();

    for _ in 0..HARD_TURN_LIMIT {
        if let Some(reason) = check_budget(&metrics, start.elapsed().as_secs_f64(), budget) {
            return finalize(model, metrics, start, Outcome::BudgetExceeded(reason));
        }
        let req = MessagesRequest {
            model: model.to_string(),
            max_tokens: budget.max_tokens.map(|t| t as u32).unwrap_or(DEFAULT_MAX_TOKENS),
            system: system_blocks.clone(),
            messages: messages.clone(),
            tools: tools.clone(),
            tool_choice: None,
        };
        let resp = match call_with_retry(auth, http, &req) {
            Ok(r) => r,
            Err(e) => return finalize(model, metrics, start, Outcome::ApiError(e)),
        };
        metrics.api_calls += 1;
        accum_usage(&mut metrics.usage, &resp.usage);
        messages.push(Message { role: "assistant".to_string(), content: resp.content.clone() });

        let tool_uses = resp.tool_uses();
        if tool_uses.is_empty() {
            return finalize(model, metrics, start, Outcome::NoToolUse);
        }

        // 順に apply して tool_result を集める。terminal が出たら break。
        let (results, terminal) = run_tool_uses(&tool_uses, &mut metrics, apply_fn);
        messages.push(Message { role: "user".to_string(), content: results });
        if let Some(t) = terminal {
            return finalize(model, metrics, start, t);
        }
    }
    finalize(
        model,
        metrics,
        start,
        Outcome::BudgetExceeded(format!("ハードターン上限 {HARD_TURN_LIMIT} に到達")),
    )
}

fn finalize(
    model: &str,
    mut metrics: ApiWorkerMetrics,
    start: Instant,
    outcome: Outcome,
) -> (Outcome, ApiWorkerMetrics) {
    metrics.wall_seconds = start.elapsed().as_secs_f64();
    metrics.cost_usd = estimate_cost_usd(model, &metrics.usage);
    (outcome, metrics)
}

fn check_budget(m: &ApiWorkerMetrics, wall: f64, b: &Budget) -> Option<String> {
    if let Some(max) = b.max_tool_calls {
        if m.tool_calls >= max {
            return Some(format!("max_tool_calls={max} に到達"));
        }
    }
    if let Some(max) = b.max_tokens {
        let used = m.usage.input_tokens + m.usage.output_tokens;
        if used >= max {
            return Some(format!("max_tokens={max} に到達（used={used}）"));
        }
    }
    if let Some(max) = b.max_wall_seconds {
        if wall >= max as f64 {
            return Some(format!("max_wall_seconds={max} に到達"));
        }
    }
    None
}

fn accum_usage(into: &mut Usage, add: &Usage) {
    into.input_tokens += add.input_tokens;
    into.output_tokens += add.output_tokens;
    into.cache_creation_input_tokens += add.cache_creation_input_tokens;
    into.cache_read_input_tokens += add.cache_read_input_tokens;
}
