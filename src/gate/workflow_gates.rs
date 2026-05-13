//! workflow.toml の append-only 制約 gate（`workflow_append_only`）。
//!
//! 本体の diff ロジックは `workflow_diff` モジュール（`docs/schemas.md` §3 / `DESIGN.md` §5.1）。

use super::{GateCtx, GateResult};
use crate::gate::workflow_diff::diff_check;
use crate::workflow::Workflow;

/// run 開始時の workflow.toml スナップショットと現 workflow.toml の差分が
/// 「追加のみ」（新規ノード・未到達ノードへの配線追加・exit_gates 追加・tools 縮小 等）
/// に収まるかを検証する。違反があれば具体的な note とともに fail。
pub(super) fn workflow_append_only(ctx: &GateCtx) -> GateResult {
    let Some(cur) = ctx.workflow else {
        return GateResult::fail("workflow が ctx に無い");
    };
    let Some(snap_text) = ctx.workflow_snapshot else {
        return GateResult::fail("workflow スナップショットが無い（run 開始時の workflow.toml）");
    };
    let snap: Workflow = match toml::from_str(snap_text) {
        Ok(w) => w,
        Err(e) => return GateResult::fail(format!("スナップショットのパース失敗: {e}")),
    };
    let v = diff_check(&snap, cur, ctx.reached_nodes);
    if v.ok {
        GateResult::ok(v.note)
    } else {
        GateResult::fail(v.note)
    }
}
