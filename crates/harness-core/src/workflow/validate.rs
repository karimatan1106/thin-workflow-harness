//! workflow.toml の最小静的検証。

use std::collections::{HashMap, HashSet};

use crate::gate::known_gates;
use crate::spec::Spec;

use super::Workflow;

/// `next` 参照グラフに循環があるか DFS で検出し、検出したエッジの説明列を返す。
/// `branches` / `wait` は fork/join 用で循環の意味が異なるため対象外（最小修正）。
fn detect_cycle_edges(wf: &Workflow) -> Vec<String> {
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for n in &wf.node {
        adj.insert(n.id.as_str(), n.next.iter().map(|s| s.as_str()).collect());
    }
    let mut visited: HashSet<&str> = HashSet::new();
    let mut on_stack: HashSet<&str> = HashSet::new();
    let mut found: Vec<String> = Vec::new();
    fn dfs<'a>(
        node: &'a str,
        adj: &HashMap<&'a str, Vec<&'a str>>,
        visited: &mut HashSet<&'a str>,
        on_stack: &mut HashSet<&'a str>,
        found: &mut Vec<String>,
    ) {
        if !visited.insert(node) {
            return;
        }
        on_stack.insert(node);
        if let Some(nexts) = adj.get(node) {
            for nx in nexts {
                if on_stack.contains(nx) {
                    let msg = format!("ノード '{node}' の next '{nx}' で循環");
                    if !found.contains(&msg) {
                        found.push(msg);
                    }
                } else if !visited.contains(nx) {
                    dfs(nx, adj, visited, on_stack, found);
                }
            }
        }
        on_stack.remove(node);
    }
    for n in &wf.node {
        if !visited.contains(n.id.as_str()) {
            dfs(n.id.as_str(), &adj, &mut visited, &mut on_stack, &mut found);
        }
    }
    found
}

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
    for c in detect_cycle_edges(wf) {
        errs.push(c);
    }
    errs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::{Node, Workflow, WorkflowMeta};

    fn mk_node(id: &str, next: Vec<&str>) -> Node {
        let mut t: Node = toml::from_str(&format!(
            "id = \"{id}\"\nskill = \"x.md\"\nnext = {:?}\n",
            next
        ))
        .unwrap();
        t.next = next.into_iter().map(|s| s.to_string()).collect();
        t
    }

    #[test]
    fn cycle_a_b_a_detected() {
        let wf = Workflow {
            meta: WorkflowMeta {
                name: "t".into(),
                entry: "a".into(),
                mandatory_gates: vec![],
                host: None,
                default_model: None,
                default_budget: None,
                run_cost_budget: None,
                secrets_glob: vec![],
            },
            node: vec![mk_node("a", vec!["b"]), mk_node("b", vec!["a"])],
        };
        let errs = validate(&wf, None);
        assert!(
            errs.iter().any(|e| e.contains("循環")),
            "expected cycle error, got: {:?}",
            errs
        );
    }

    #[test]
    fn no_cycle_passes() {
        let wf = Workflow {
            meta: WorkflowMeta {
                name: "t".into(),
                entry: "a".into(),
                mandatory_gates: vec![],
                host: None,
                default_model: None,
                default_budget: None,
                run_cost_budget: None,
                secrets_glob: vec![],
            },
            node: vec![mk_node("a", vec!["b"]), mk_node("b", vec![])],
        };
        let errs = validate(&wf, None);
        assert!(
            !errs.iter().any(|e| e.contains("循環")),
            "unexpected cycle in linear graph: {:?}",
            errs
        );
    }
}
