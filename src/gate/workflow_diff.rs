//! `workflow_append_only` の本体 ── run 開始時スナップショットと現 workflow.toml の proper diff。
//!
//! 許可される差分（`docs/schemas.md` §3 / `DESIGN.md` §5.1）:
//! (a) 既存ノード id が全て残っている
//! (b) 各既存ノードの `exit_gates` が run-start の superset（追加可・削除/弱体化不可）
//! (c) `on_reject` が変更なし or 厳格化（`after` が同じか小さい、`goto` が同じ）
//! (d) `tools` が run-start の subset（縮小のみ）
//! (e) `context.include` が変更なし or 縮小
//! (f) `[meta].entry` が変更なし
//! (g) 新規ノードは `[meta].mandatory_gates` の各 gate を `exit_gates` に含む
//! (h) 既存ノードの `next`/`branches`/`wait` への追加は「まだ到達してないノード」へのみ
//!     （到達済みは advance イベントの from/to から判定。skeleton では `reached_nodes` で渡される）

use std::collections::HashSet;

use crate::workflow::{GateSpec, Node, Workflow};

/// diff 検証の結果（ok=false なら note に具体的な違反内容）。
pub struct DiffVerdict {
    pub ok: bool,
    pub note: String,
}

fn fail(note: impl Into<String>) -> DiffVerdict {
    DiffVerdict { ok: false, note: note.into() }
}

fn gate_names(gates: &[GateSpec]) -> HashSet<&str> {
    gates.iter().map(|g| g.gate.as_str()).collect()
}

/// gate spec を「名前 + 引数」でラフに比較する（弱体化検出 ── 引数が変わったら弱体化扱い）。
fn gate_sig(g: &GateSpec) -> (String, String) {
    let args = serde_json::to_string(&g.args_table()).unwrap_or_default();
    (g.gate.clone(), args)
}

fn ctx_include(node: &Node) -> Vec<String> {
    node.context.as_ref().map(|c| c.include.clone()).unwrap_or_default()
}

/// 既存ノード `old`（snapshot 時点）と `cur`（現在）を比較する。
fn check_existing_node(old: &Node, cur: &Node, reached: &HashSet<&str>) -> DiffVerdict {
    // (b) exit_gates: snapshot の各 gate spec（名前＋args）が現在も残っていること。
    let cur_sigs: HashSet<(String, String)> = cur.exit_gates.iter().map(gate_sig).collect();
    for og in &old.exit_gates {
        if !cur_sigs.contains(&gate_sig(og)) {
            return fail(format!(
                "ノード '{}' の exit_gate '{}' が削除/弱体化された",
                old.id, og.gate
            ));
        }
    }
    // (c) on_reject: 変更なし or 厳格化（after <= 旧、goto 同一）。
    match (&old.on_reject, &cur.on_reject) {
        (Some(o), Some(c)) => {
            if c.goto != o.goto {
                return fail(format!("ノード '{}' の on_reject.goto が変更された", old.id));
            }
            if c.after > o.after {
                return fail(format!(
                    "ノード '{}' の on_reject.after が緩和された ({} → {})",
                    old.id, o.after, c.after
                ));
            }
        }
        (Some(_), None) => return fail(format!("ノード '{}' の on_reject が削除された（緩和）", old.id)),
        // None → Some は厳格化（リトライ方針の追加）なので許可。
        _ => {}
    }
    // (d) tools: 現 tools が snapshot tools の subset（縮小のみ）。snapshot が空なら「無制限」基準。
    if !old.tools.is_empty() {
        let old_tools: HashSet<&str> = old.tools.iter().map(|s| s.as_str()).collect();
        for t in &cur.tools {
            if !old_tools.contains(t.as_str()) {
                return fail(format!("ノード '{}' にツール '{t}' が追加された（拡大）", old.id));
            }
        }
    }
    // (e) context.include: 現 include が snapshot include の subset（縮小のみ）。
    let (oi, ci) = (ctx_include(old), ctx_include(cur));
    if !oi.is_empty() {
        let oset: HashSet<&str> = oi.iter().map(|s| s.as_str()).collect();
        for e in &ci {
            if !oset.contains(e.as_str()) {
                return fail(format!("ノード '{}' の context に '{e}' が追加された（拡大）", old.id));
            }
        }
    }
    // (h) next/branches/wait への配線追加 ── 「このノードが未到達」のときだけ許可。
    if reached.contains(old.id.as_str()) {
        let wire_changed = old.next != cur.next || old.branches != cur.branches || old.wait != cur.wait;
        if wire_changed {
            return fail(format!(
                "到達済みノード '{}' の next/branches/wait が変更された（到達後の配線変更は不可）",
                old.id
            ));
        }
    } else {
        // 未到達ノード ── 既存配線が消えていないこと（追加のみ可）。
        for x in &old.next {
            if !cur.next.contains(x) {
                return fail(format!("未到達ノード '{}' の next から '{x}' が削除された", old.id));
            }
        }
        for x in &old.branches {
            if !cur.branches.contains(x) {
                return fail(format!("未到達ノード '{}' の branches から '{x}' が削除された", old.id));
            }
        }
        for x in &old.wait {
            if !cur.wait.contains(x) {
                return fail(format!("未到達ノード '{}' の wait から '{x}' が削除された", old.id));
            }
        }
    }
    DiffVerdict { ok: true, note: String::new() }
}

/// snapshot と現 workflow を比較し、append-only に収まるかを判定する。
pub fn diff_check(snap: &Workflow, cur: &Workflow, reached_nodes: &[String]) -> DiffVerdict {
    // (f) entry 不変。
    if cur.meta.entry != snap.meta.entry {
        return fail(format!("[meta].entry が変更された: {} → {}", snap.meta.entry, cur.meta.entry));
    }
    let reached: HashSet<&str> = reached_nodes.iter().map(|s| s.as_str()).collect();
    let cur_ids: HashSet<&str> = cur.node.iter().map(|n| n.id.as_str()).collect();
    let snap_ids: HashSet<&str> = snap.node.iter().map(|n| n.id.as_str()).collect();

    // (a) 既存ノードが全て残っている。
    for old in &snap.node {
        let Some(cur_node) = cur.node.iter().find(|n| n.id == old.id) else {
            return fail(format!("既存ノード '{}' が削除された", old.id));
        };
        let v = check_existing_node(old, cur_node, &reached);
        if !v.ok {
            return v;
        }
    }

    // (g) 新規ノードは mandatory_gates を満たす。
    let mand: Vec<(String, String)> = cur.meta.mandatory_gates.iter().map(gate_sig).collect();
    let mand_names = gate_names(&cur.meta.mandatory_gates);
    for n in &cur.node {
        if snap_ids.contains(n.id.as_str()) {
            continue;
        }
        let ng: HashSet<(String, String)> = n.exit_gates.iter().map(gate_sig).collect();
        let ng_names = gate_names(&n.exit_gates);
        for (i, m) in cur.meta.mandatory_gates.iter().enumerate() {
            let sig = &mand[i];
            // 名前一致 + 引数一致を要求（mandatory は引数まで固定の意図）。ただし
            // 引数なしの mandatory（traceability_closed 等）は名前一致だけで十分。
            let satisfied = if m.args_table().is_empty() {
                ng_names.contains(m.gate.as_str())
            } else {
                ng.contains(sig)
            };
            if !satisfied {
                return fail(format!(
                    "新規ノード '{}' に mandatory gate '{}' が無い（または引数不一致）",
                    n.id, m.gate
                ));
            }
        }
        let _ = mand_names;
    }

    // 新規配線の妥当性（既存ノードの next が未到達ノードを指すか）は check_existing_node で
    // 「到達済みノードの配線変更不可」として捕まえる。新規ノードの next が新規/未到達を指すのは可。
    let _ = cur_ids;
    DiffVerdict {
        ok: true,
        note: format!(
            "workflow は append-only（既存 {} ノード保持、新規 {} ノード）",
            snap.node.len(),
            cur.node.len() - snap.node.len()
        ),
    }
}
