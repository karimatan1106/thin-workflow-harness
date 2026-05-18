//! workflow.toml の最小静的検証。

use std::collections::HashSet;

use crate::gate::known_gates;
use crate::spec::Spec;

use super::Workflow;

/// 最小の静的検証。エラーメッセージの Vec を返す（空なら OK）。
pub fn validate(wf: &Workflow, _spec: Option<&Spec>) -> Vec<String> {
    let mut errs = Vec::new();
    let ids: HashSet<&str> = wf.node.iter().map(|n| n.id.as_str()).collect();

    if !ids.contains(wf.meta.entry.as_str()) {
        errs.push(format!("entry '{}' が実在ノード id でない", wf.meta.entry));
    }
    if wf.node.is_empty() {
        errs.push("ノードが 1 つも無い".to_string());
    }

    let known = known_gates();
    let check_gate = |where_: &str, g: &str, errs: &mut Vec<String>| {
        if !known.contains(&g) {
            errs.push(format!("{where_} に未知の gate: {g}"));
        }
    };
    for gs in &wf.meta.mandatory_gates {
        check_gate("mandatory_gates", &gs.gate, &mut errs);
    }
    for n in &wf.node {
        for nx in n.next.iter().chain(n.branches.iter()).chain(n.wait.iter()) {
            if !ids.contains(nx.as_str()) {
                errs.push(format!("ノード '{}' の参照 '{nx}' が実在ノード id でない", n.id));
            }
        }
        for gs in &n.exit_gates {
            check_gate(&format!("ノード '{}'", n.id), &gs.gate, &mut errs);
        }
        for at in &n.artifact_tags {
            for gs in &at.gates {
                check_gate(&format!("ノード '{}' tag '{}'", n.id, at.tag), &gs.gate, &mut errs);
            }
        }
        if let Some(orj) = &n.on_reject {
            if orj.goto != "__human__" && !ids.contains(orj.goto.as_str()) {
                errs.push(format!(
                    "ノード '{}' の on_reject.goto '{}' が実在ノード id でない",
                    n.id, orj.goto
                ));
            }
        }
        if n.node_type() == "task" && n.skill.is_none() {
            errs.push(format!("task ノード '{}' に skill が無い", n.id));
        }
    }
    errs
}
