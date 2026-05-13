//! ノードループ ── 1 run の spawn 反復、ApiWorker 駆動、metrics サイドカー書き込み。
//!
//! CLI ハンドラ（`api_run.rs`）と分離: ここは「準備済み deps」を受け取り純粋にループを回す。
//! テストでは `max_spawns` を小さく上書きできる（per-instance フィールド化）。
//!
//! ファイル分割:
//! - `mod.rs` ── `RunnerDeps` と `run_loop` 本体（spawn 反復・metrics 出力）
//! - `apply_dispatch.rs` ── 1 ツール呼び出しの apply
//! - `guards.rs` ── 暴走防止（`BudgetStreak` / cache_create=0 warning / max_tokens 推奨値）
//!
//! ## 暴走防止の二段構え
//! - `max_spawns`（既定 10）── 1 run 全体の spawn 回数の絶対上限。
//! - 連続 BudgetExceeded early-fail ── 同ノードで `BudgetExceeded` が 2 回続いたら即終了し、
//!   「budget.max_tokens を増やせ（現在 X、推奨 2X）」を具体値で人間にエスカレ。
//! - cache_create=0 warning ── 1 run で 1 度だけ「cache 未作成」を警告（早期検知）。

mod apply_dispatch;
mod guards;

use std::path::PathBuf;
use std::time::Instant;

use crate::event::{append_event, read_events, EventKind};
use crate::handlers::state_for;
use crate::handlers2::RunCtx;
use crate::metrics::{append_metrics, NodeMetrics, TokenBreakdown};
use crate::runtime::api_worker::{ApiWorker, ApplyResult, Outcome};
use crate::runtime::apply::{has_pending_escalation, rejected_since_transition};
use crate::runtime::auth::AuthMode;
use crate::runtime::context;
use crate::runtime::http_client::HttpClient;
use crate::runtime::interceptor::Interceptor;
use crate::runtime::tools::ToolCall;
use crate::workflow::{current_node, Workflow};

use apply_dispatch::{apply_one, effective_budget};
use guards::{cache_warning_message, recommend_max_tokens, should_emit_cache_warning, BudgetStreak};

/// ノードループ実行に必要な依存をまとめた struct。
pub struct RunnerDeps<'a> {
    pub run_id: String,
    pub wf: Workflow,
    pub cwd: PathBuf,
    pub model_default: String,
    pub http: &'a dyn HttpClient,
    pub auth: AuthMode,
    /// 暴走防止 ── spawn 最大回数（既定 10、test では小さく上書き可）。
    pub max_spawns: usize,
}

/// ノードループ本体。
pub fn run_loop(d: RunnerDeps<'_>) -> Result<(), String> {
    let RunnerDeps { run_id, wf, cwd, model_default, http, auth, max_spawns } = d;
    let mut budget_streak = BudgetStreak::new();
    let mut cache_warning_emitted = false;
    for spawn_n in 1..=max_spawns {
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

        // cache 未作成 → 1 run で 1 度警告（真因: system+tools が 1024 token 閾値未達 等）。
        if !cache_warning_emitted && should_emit_cache_warning(&metrics) {
            eprintln!("{}", cache_warning_message());
            cache_warning_emitted = true;
        }

        // 同ノードで BudgetExceeded が連続したら early-fail（spawn 上限を待たない）。
        budget_streak.enter(&node.id);
        if budget_streak.observe_outcome(&outcome) {
            let rec = recommend_max_tokens(&budget);
            return Err(format!(
                "run {run_id}: ノード '{}' で BudgetExceeded が {} 連続 ── 早期打ち切り。\n{rec}\n\
                 ヒント: workflow.toml の `[meta.default_budget]` または `[[node]].budget` で max_tokens を上げる。",
                node.id, budget_streak.count()
            ));
        }

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
    Err(format!("run spawn 上限 {max_spawns} に到達"))
}
