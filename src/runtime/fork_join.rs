//! fork/join parallel execution.
//! See DESIGN.md for the spec; this is the Phase 2 minimum viable.
//!
//! Each branch writes events to state/<run-id>.<branch_id>.jsonl (sub-log).
//! On success, derive_state folds branch artifacts/evidence into main state.

use std::thread;


use crate::event::{append_event, EventKind};
use crate::gate::{eval_gate, GateCtx};
use crate::handlers::state_for;
use crate::handlers2::RunCtx;
use crate::paths;
use crate::runtime::script::Step;
use crate::workflow::Workflow;
use crate::runtime::fork_join_branch::branch_thread;

/// Drive fork node branches in parallel (scripted worker only).
pub fn run_parallel_scripted(
    run_id: &str,
    wf: &Workflow,
    fork_node: &crate::workflow::Node,
    steps: Vec<Step>,
) -> Result<(), String> {
    let branches: Vec<String> = fork_node.branches.clone();
    if branches.len() < 2 {
        return Err(format!(
            "fork node {} has fewer than 2 branches; cannot parallelize",
            fork_node.id
        ));
    }
    check_pairwise_disjoint(run_id, wf, &branches)?;

    append_event(run_id, EventKind::BranchForked { branch_ids: branches.clone() })
        .map_err(|e| format!("branch_forked write fail: {e}"))?;
    println!("[fork {}] spawning branches={:?}", fork_node.id, branches);

    let mut handles = Vec::new();
    for bid in &branches {
        let bid_owned = bid.clone();
        let run_owned = run_id.to_string();
        let steps_for_branch: Vec<Step> = steps
            .iter()
            .filter(|s| s.node == bid_owned)
            .cloned()
            .collect();
        let h = thread::Builder::new()
            .name(format!("branch-{bid_owned}"))
            .spawn(move || branch_thread(&run_owned, &bid_owned, steps_for_branch))
            .map_err(|e| format!("branch thread spawn fail ({bid}): {e}"))?;
        handles.push((bid.clone(), h));
    }

    let mut failures: Vec<String> = Vec::new();
    for (bid, h) in handles {
        match h.join() {
            Ok(Ok(())) => println!("[branch {bid}] done"),
            Ok(Err(e)) => {
                println!("[branch {bid}] fail: {e}");
                failures.push(format!("{bid}: {e}"));
            }
            Err(_) => {
                println!("[branch {bid}] panic");
                failures.push(format!("{bid}: panic"));
            }
        }
    }

    if !failures.is_empty() {
        append_event(
            run_id,
            EventKind::BranchJoined {
                branch_ids: branches.clone(),
                status: "failed".into(),
                failures: Some(failures.clone()),
            },
        )
        .map_err(|e| format!("branch_joined write fail: {e}"))?;
        return Err(format!(
            "fork {}: {} branch(es) failed: {}",
            fork_node.id,
            failures.len(),
            failures.join(" / ")
        ));
    }
    append_event(
        run_id,
        EventKind::BranchJoined {
            branch_ids: branches.clone(),
            status: "success".into(),
            failures: None,
        },
    )
    .map_err(|e| format!("branch_joined write fail: {e}"))?;

    let st = state_for(run_id, wf)?;
    // fork.next[0] が指定されていればそこへ非自然遷移（fork→jn 直行）。
    // 未指定（既存 fixture）は phase_index+1 ── branches[0] へ進む旧挙動互換。
    let to = if let Some(nxt) = fork_node.next.first() {
        nxt.clone()
    } else {
        wf.nodes()
            .get(st.phase_index + 1)
            .map(|n| n.id.clone())
            .unwrap_or_else(|| "(done)".to_string())
    };
    append_event(run_id, EventKind::Advance { from: fork_node.id.clone(), to: to.clone() })
        .map_err(|e| format!("advance write fail: {e}"))?;
    println!("[fork {}] -> advance -> {to} (all branches ok)", fork_node.id);
    Ok(())
}

pub(crate) fn check_pairwise_disjoint(run_id: &str, wf: &Workflow, branches: &[String]) -> Result<(), String> {
    let rc = RunCtx::load(run_id);
    let st = state_for(run_id, wf)?;
    let home = paths::harness_home();
    let ctx = GateCtx {
        home: &home,
        workflow: Some(wf),
        workflow_snapshot: rc.snapshot.as_deref(),
        spec: rc.spec.as_ref(),
        questions: &rc.questions,
        current_node: None,
        reached_nodes: &rc.reached,
    };
    for i in 0..branches.len() {
        for j in (i + 1)..branches.len() {
            let mut args = toml::Table::new();
            args.insert("node_a".into(), toml::Value::String(branches[i].clone()));
            args.insert("node_b".into(), toml::Value::String(branches[j].clone()));
            let r = eval_gate("blast_radius_disjoint", &args, &st, &ctx);
            if !r.ok {
                return Err(format!(
                    "blast_radius_disjoint violation: {} vs {} -- {}",
                    branches[i], branches[j], r.note
                ));
            }
        }
    }
    Ok(())
}
