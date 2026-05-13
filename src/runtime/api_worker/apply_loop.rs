//! tool_use の apply 反復 ── 1 ターンの assistant 応答にある複数 tool_use を順番に
//! `apply_fn` に渡し、`ToolResult` ブロックの列を組み立てる。

use crate::runtime::anthropic::ContentBlock;
use crate::runtime::tools::tool_use_to_call;

use super::{ApiWorkerMetrics, ApplyResult, Outcome};

/// `tool_uses`（1 ターン分）を順番に処理する。terminal アクションに当たったらそこで break。
pub(super) fn run_tool_uses(
    tool_uses: &[(&str, &str, &serde_json::Value)],
    metrics: &mut ApiWorkerMetrics,
    apply_fn: &mut dyn FnMut(crate::runtime::tools::ToolCall) -> ApplyResult,
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
