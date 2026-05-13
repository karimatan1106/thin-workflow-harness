//! `ApiWorker` ── 生 Anthropic API を直叩きする本物の LLM worker（`DESIGN.md` §10 の "工夫 3"）。
//!
//! tool-use ループ:
//!   1. 初期メッセージ = `WorkerContext` を文字列化（system block = cache prefix、status/feedback = 可変サフィックス）。
//!   2. POST `/v1/messages` → 返ってきた `ToolUse` ブロックを harness の `ToolCall` にマップして 1 つずつ apply。
//!   3. 結果（成功時は短い ok 文、失敗時は理由文）を `ToolResult` ブロックにし、次の user メッセージに含めて再 POST。
//!   4. assistant が `request_transition` / `back` / `stuck` を呼んだら終了。`stop_reason="end_turn"` で tool_use 無しなら詰まり扱い。
//!
//! prompt caching: system block と「skill + spec スライス」block に `cache_control:ephemeral` を付ける ──
//! 同一ノード内のリトライで cache hit する。可変サフィックス（status / feedback）には付けない。

use std::time::Instant;

use crate::runtime::anthropic::{
    estimate_cost_usd, CacheControl, ContentBlock, Message, MessagesRequest, MessagesResponse,
    Usage,
};
use crate::runtime::auth::AuthMode;
use crate::runtime::http_client::{HttpClient, HttpResponse};
use crate::runtime::tools::{tool_defs, tool_use_to_call, ToolCall};
use crate::runtime::worker::WorkerContext;
use crate::workflow::Budget;

/// API URL とヘッダの定数（`docs/implementation.md`）。
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;
/// 1 ノードあたりリクエスト回数の絶対上限（暴走防止 ── ノード budget の保険）。
const HARD_TURN_LIMIT: usize = 64;

/// `ApiWorker.drive` の最終結果。
#[derive(Debug, Clone)]
pub enum Outcome {
    /// assistant が `request_transition` を呼んだ。
    Transitioned,
    /// `back` を呼んだ。
    WentBack(String),
    /// `stuck` を呼んだ。
    Stuck(String),
    /// budget 超過で打ち切り（`node_aborted{reason:budget}` 相当）。
    BudgetExceeded(String),
    /// API リトライも尽きた（`node_aborted{reason:api_error}` 相当）。
    ApiError(String),
    /// `stop_reason="end_turn"` で tool_use 無し ── 詰まり。
    NoToolUse,
}

/// 1 ツール呼び出しの apply 結果 ── tool_result に詰めて会話に戻す。
pub struct ApplyResult {
    pub content: String,
    pub is_error: bool,
    /// terminal（transition / back / stuck）に達したらここに乗せる。
    pub terminal: Option<Outcome>,
}

/// 集約メトリクス（ノード分の tokens / wall / API 呼び出し回数）。
#[derive(Debug, Clone, Default)]
pub struct ApiWorkerMetrics {
    pub api_calls: u64,
    pub tool_calls: u64,
    pub wall_seconds: f64,
    pub usage: Usage,
    pub cost_usd: Option<f64>,
}

/// 本物の LLM worker。`http` は `&dyn` で受けるので 1 個のクライアントを複数 spawn で共有できる。
pub struct ApiWorker<'a> {
    auth: AuthMode,
    model: String,
    http: &'a dyn HttpClient,
}

impl<'a> ApiWorker<'a> {
    pub fn new(auth: AuthMode, model: String, http: &'a dyn HttpClient) -> Self {
        ApiWorker { auth, model, http }
    }

    /// ノード 1 つを動かす。`apply_fn` は worker が決めたツール呼び出しを harness 側に適用する callback。
    pub fn drive(
        &self,
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
                return finalize(&self.model, metrics, start, Outcome::BudgetExceeded(reason));
            }
            let req = MessagesRequest {
                model: self.model.clone(),
                max_tokens: budget.max_tokens.map(|t| t as u32).unwrap_or(DEFAULT_MAX_TOKENS),
                system: system_blocks.clone(),
                messages: messages.clone(),
                tools: tools.clone(),
                tool_choice: None,
            };
            let resp = match self.call_with_retry(&req) {
                Ok(r) => r,
                Err(e) => return finalize(&self.model, metrics, start, Outcome::ApiError(e)),
            };
            metrics.api_calls += 1;
            accum_usage(&mut metrics.usage, &resp.usage);
            messages.push(Message { role: "assistant".to_string(), content: resp.content.clone() });

            let tool_uses = resp.tool_uses();
            if tool_uses.is_empty() {
                return finalize(&self.model, metrics, start, Outcome::NoToolUse);
            }

            // 順に apply して tool_result を集める。terminal が出たら break。
            let (results, terminal) = run_tool_uses(&tool_uses, &mut metrics, apply_fn);
            messages.push(Message { role: "user".to_string(), content: results });
            if let Some(t) = terminal {
                return finalize(&self.model, metrics, start, t);
            }
        }
        finalize(
            &self.model,
            metrics,
            start,
            Outcome::BudgetExceeded(format!("ハードターン上限 {HARD_TURN_LIMIT} に到達")),
        )
    }

    /// 429/5xx は指数バックオフでリトライ（最大 3 回）、その他 4xx は即 fail。
    fn call_with_retry(&self, req: &MessagesRequest) -> Result<MessagesResponse, String> {
        let body = serde_json::to_string(req).map_err(|e| format!("リクエスト直列化失敗: {e}"))?;
        let mut headers = self.auth.auth_headers(API_VERSION);
        headers.push(("content-type".to_string(), "application/json".to_string()));
        let mut last_err = String::new();
        for attempt in 0..4 {
            match self.http.post(API_URL, &headers, &body) {
                Ok(HttpResponse { status: 200, body: text }) => {
                    return serde_json::from_str::<MessagesResponse>(&text)
                        .map_err(|e| format!("レスポンスパース失敗: {e} ── body={text}"));
                }
                Ok(HttpResponse { status, body: text })
                    if (500..=599).contains(&status) || status == 429 =>
                {
                    last_err = format!("HTTP {status}: {text}");
                    if attempt < 3 {
                        std::thread::sleep(std::time::Duration::from_millis(200 << attempt));
                        continue;
                    }
                }
                Ok(HttpResponse { status, body: text }) => {
                    return Err(format!("HTTP {status}: {text}"));
                }
                Err(e) => {
                    last_err = format!("transport: {e}");
                    if attempt < 3 {
                        std::thread::sleep(std::time::Duration::from_millis(200 << attempt));
                        continue;
                    }
                }
            }
        }
        Err(format!("API リトライ尽きた: {last_err}"))
    }
}

fn run_tool_uses(
    tool_uses: &[(&str, &str, &serde_json::Value)],
    metrics: &mut ApiWorkerMetrics,
    apply_fn: &mut dyn FnMut(ToolCall) -> ApplyResult,
) -> (Vec<ContentBlock>, Option<Outcome>) {
    let mut results: Vec<ContentBlock> = Vec::new();
    let mut terminal: Option<Outcome> = None;
    for (tu_id, tu_name, tu_input) in tool_uses {
        metrics.tool_calls += 1;
        let call = match tool_use_to_call(tu_name, tu_input) {
            Ok(c) => c,
            Err(e) => {
                results.push(ContentBlock::ToolResult {
                    tool_use_id: tu_id.to_string(),
                    content: format!("tool 引数エラー: {e}"),
                    is_error: Some(true),
                });
                continue;
            }
        };
        let out = apply_fn(call);
        results.push(ContentBlock::ToolResult {
            tool_use_id: tu_id.to_string(),
            content: out.content,
            is_error: if out.is_error { Some(true) } else { None },
        });
        if let Some(t) = out.terminal {
            terminal = Some(t);
            break;
        }
    }
    (results, terminal)
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
