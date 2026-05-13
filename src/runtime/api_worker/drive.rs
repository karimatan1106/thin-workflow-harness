//! drive ループ本体 ── `ApiWorker::drive` の中身。

use std::time::Instant;

use crate::runtime::anthropic::{
    estimate_cost_usd, CacheControl, ContentBlock, Message, MessagesRequest, Usage,
};
use crate::runtime::auth::AuthMode;
use crate::runtime::http_client::HttpClient;
use crate::runtime::tools::{tool_defs, ToolCall};
use crate::runtime::worker::WorkerContext;
use crate::workflow::Budget;

use super::apply_loop::run_tool_uses;
use super::retry::call_with_retry;
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

/// system を「静的本文」「skill + spec スライス」の 2 ブロックに割り、両方に cache_control を付ける。
fn build_system_blocks(ctx: &WorkerContext) -> Vec<ContentBlock> {
    let mut out = Vec::new();
    if !ctx.system_prompt.is_empty() {
        out.push(ContentBlock::Text {
            text: ctx.system_prompt.clone(),
            cache_control: Some(CacheControl::ephemeral()),
        });
    }
    let mut prefix = String::new();
    if !ctx.skill_body.is_empty() {
        prefix.push_str("# ノード skill\n");
        prefix.push_str(&ctx.skill_body);
        prefix.push('\n');
    }
    if !ctx.spec_slice.is_empty() {
        prefix.push_str("\n# spec スライス\n");
        prefix.push_str(&ctx.spec_slice);
        prefix.push('\n');
    }
    if !prefix.is_empty() {
        out.push(ContentBlock::Text {
            text: prefix,
            cache_control: Some(CacheControl::ephemeral()),
        });
    }
    out
}

/// 初期 user メッセージ ── ノードヘッダ + 可変サフィックス（status, feedback）。
fn initial_user_message(ctx: &WorkerContext) -> Message {
    let mut text = String::new();
    text.push_str(&format!("# 現ノード\n{}\n\n", ctx.node_header));
    text.push_str("# 現在の status（harness 観測）\n");
    text.push_str(&ctx.compact_status);
    text.push('\n');
    if ctx.is_respawn() {
        text.push_str("\n# 直前 advance_rejected の failed_gates（再 spawn フィードバック）\n");
        for (g, r) in &ctx.failed_gates {
            text.push_str(&format!("- {g}: {r}\n"));
        }
    }
    text.push_str("\n# 渡されたツール\n");
    text.push_str(&ctx.tools.join(", "));
    text.push('\n');
    text.push_str("\n作業が完了したら `request_transition` を呼ぶ。詰まったら `stuck`。要件不足なら `back`。\n");
    Message {
        role: "user".to_string(),
        content: vec![ContentBlock::Text { text, cache_control: None }],
    }
}

#[cfg(test)]
pub(crate) fn test_initial_user_message_text(ctx: &WorkerContext) -> String {
    let m = initial_user_message(ctx);
    match &m.content[0] {
        ContentBlock::Text { text, .. } => text.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> WorkerContext {
        WorkerContext {
            system_prompt: "you are worker".into(),
            node_header: "n1 (implement)".into(),
            skill_body: "skill body".into(),
            spec_slice: "F-001 do X".into(),
            compact_status: "node 1/2 n1".into(),
            failed_gates: vec![],
            tools: vec!["read".into(), "edit".into()],
        }
    }

    #[test]
    fn system_blocks_carry_cache_control() {
        let bs = build_system_blocks(&ctx());
        assert_eq!(bs.len(), 2);
        for b in &bs {
            match b {
                ContentBlock::Text { cache_control, .. } => assert!(cache_control.is_some()),
                _ => panic!("not text"),
            }
        }
    }

    #[test]
    fn initial_user_includes_status_and_tools() {
        let text = test_initial_user_message_text(&ctx());
        assert!(text.contains("n1 (implement)"));
        assert!(text.contains("node 1/2 n1"));
        assert!(text.contains("read, edit"));
    }
}
