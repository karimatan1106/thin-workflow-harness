//! workflow.toml の append-only 制約系 gate。

use std::collections::HashSet;

use super::{GateCtx, GateResult};
use crate::workflow::Workflow;

/// workflow_append_only（簡易検証）: run 開始時のノード id 集合 ⊆ 現ノード id 集合、
/// かつ各既存ノードの exit_gates が削られていない（superset）。完全な diff 検証は後フェーズ。
pub(super) fn workflow_append_only(ctx: &GateCtx) -> GateResult {
    let Some(cur) = ctx.workflow else {
        return GateResult::fail("workflow が ctx に無い");
    };
    let Some(snap_text) = ctx.workflow_snapshot else {
        return GateResult::fail("workflow スナップショットが無い");
    };
    let snap: Workflow = match toml::from_str(snap_text) {
        Ok(w) => w,
        Err(e) => return GateResult::fail(format!("スナップショットのパース失敗: {e}")),
    };
    if cur.meta.entry != snap.meta.entry {
        return GateResult::fail(format!("[meta].entry が変更された: {} → {}", snap.meta.entry, cur.meta.entry));
    }
    let cur_ids: HashSet<&str> = cur.node.iter().map(|n| n.id.as_str()).collect();
    for old in &snap.node {
        if !cur_ids.contains(old.id.as_str()) {
            return GateResult::fail(format!("既存ノード '{}' が削除された", old.id));
        }
        let new_node = cur.node.iter().find(|n| n.id == old.id).unwrap();
        let new_gates: HashSet<&str> = new_node.exit_gates.iter().map(|g| g.gate.as_str()).collect();
        for og in &old.exit_gates {
            if !new_gates.contains(og.gate.as_str()) {
                return GateResult::fail(format!("ノード '{}' の exit_gate '{}' が削除された", old.id, og.gate));
            }
        }
        // ツール追加禁止（縮小のみ）
        let old_tools: HashSet<&str> = old.tools.iter().map(|s| s.as_str()).collect();
        for t in &new_node.tools {
            if !old_tools.contains(t.as_str()) && !old.tools.is_empty() {
                return GateResult::fail(format!("ノード '{}' にツール '{t}' が追加された", old.id));
            }
        }
    }
    // 新規ノードは mandatory_gates を含むこと
    let mand: Vec<&str> = cur.meta.mandatory_gates.iter().map(|g| g.gate.as_str()).collect();
    let snap_ids: HashSet<&str> = snap.node.iter().map(|n| n.id.as_str()).collect();
    for n in &cur.node {
        if snap_ids.contains(n.id.as_str()) {
            continue;
        }
        let ng: HashSet<&str> = n.exit_gates.iter().map(|g| g.gate.as_str()).collect();
        for m in &mand {
            if !ng.contains(m) {
                return GateResult::fail(format!("新規ノード '{}' に mandatory gate '{m}' が無い", n.id));
            }
        }
    }
    GateResult::ok("workflow は追加のみ（簡易検証通過）")
}
