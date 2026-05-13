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
//!
//! ファイル分割:
//! - `mod.rs` ── 型 + public API（`ApiWorker::new`, `ApiWorker::drive`）
//! - `drive.rs` ── drive ループ本体（ターン回し / budget / usage 累積）
//! - `system.rs` ── system block 構築 と 初期 user メッセージ生成
//! - `apply_loop.rs` ── tool_use の apply 反復（interceptor 通って tool_result を組む）
//! - `retry.rs` ── 429/5xx の指数バックオフ retry

mod apply_loop;
mod drive;
mod retry;
mod system;

use crate::runtime::anthropic::Usage;
use crate::runtime::auth::AuthMode;
use crate::runtime::http_client::HttpClient;
use crate::runtime::tools::ToolCall;
use crate::runtime::worker::WorkerContext;
use crate::workflow::Budget;

/// API URL とヘッダの定数（`docs/implementation.md`）。
pub(crate) const API_URL: &str = "https://api.anthropic.com/v1/messages";
pub(crate) const API_VERSION: &str = "2023-06-01";
pub(crate) const DEFAULT_MAX_TOKENS: u32 = 4096;
/// 1 ノードあたりリクエスト回数の絶対上限（暴走防止 ── ノード budget の保険）。
pub(crate) const HARD_TURN_LIMIT: usize = 64;

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
        drive::drive(&self.auth, &self.model, self.http, ctx, budget, apply_fn)
    }
}

