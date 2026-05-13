//! CLI サブコマンドのハンドラ（読み取り系: validate / skill / gates）と gate 評価ヘルパ。

use std::path::PathBuf;

use crate::gate::{eval_gate, GateCtx, GateResult, Question};
use crate::handlers::{load_wf, state_for};
use crate::paths;
use crate::questions::read_questions;
use crate::spec::{load_spec, Spec};
use crate::state::State;
use crate::status_view::print_gate_lines;
use crate::workflow::{current_node, load_workflow, validate, Node, Workflow};

/// run 文脈をまとめて読み込む（spec / 質問キュー / workflow スナップショット）。
pub struct RunCtx {
    pub spec: Option<Spec>,
    pub questions: Vec<Question>,
    pub snapshot: Option<String>,
}

impl RunCtx {
    pub fn load(run_id: &str) -> RunCtx {
        let spec = load_spec(&paths::spec_path()).ok();
        let questions = read_questions(run_id).unwrap_or_default();
        let snapshot = paths::workflow_snapshot_path(run_id)
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok());
        RunCtx { spec, questions, snapshot }
    }
}

/// 現ノードの全 gate（mandatory_gates + ノード固有 exit_gates）を評価する。
pub fn eval_node_gates(
    wf: &Workflow,
    node: &Node,
    st: &State,
    rc: &RunCtx,
) -> Vec<(String, GateResult)> {
    let home = paths::harness_home();
    let ctx = GateCtx {
        home: &home,
        workflow: Some(wf),
        workflow_snapshot: rc.snapshot.as_deref(),
        spec: rc.spec.as_ref(),
        questions: &rc.questions,
        current_node: Some(node),
    };
    let mut out = Vec::new();
    for gs in wf.meta.mandatory_gates.iter().chain(node.exit_gates.iter()) {
        let r = eval_gate(&gs.gate, &gs.args_table(), st, &ctx);
        out.push((gs.gate.clone(), r));
    }
    out
}

pub fn cmd_validate(workflow_path: Option<&str>, spec_path: Option<&str>) -> Result<(), String> {
    let wf_path = workflow_path.map(PathBuf::from).unwrap_or_else(paths::workflow_path);
    let sp_path = spec_path.map(PathBuf::from).unwrap_or_else(paths::spec_path);
    let wf = load_workflow(&wf_path)?;
    let spec = if sp_path.exists() { Some(load_spec(&sp_path)?) } else { None };
    let errs = validate(&wf, spec.as_ref());
    if errs.is_empty() {
        println!("OK ({} ノード)", wf.nodes().len());
        Ok(())
    } else {
        for e in &errs {
            eprintln!("  - {e}");
        }
        Err(format!("{} 件のエラー", errs.len()))
    }
}

pub fn cmd_skill(run: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let wf = load_wf()?;
    let st = state_for(&run_id, &wf)?;
    let Some(node) = current_node(&wf, &st) else {
        return Err("既に完了している".to_string());
    };
    let Some(skill) = &node.skill else {
        return Err(format!("ノード '{}' に skill が無い", node.id));
    };
    let p = paths::skill_path(skill);
    println!("{}", p.display());
    if let Ok(body) = std::fs::read_to_string(&p) {
        for line in body.lines().take(8) {
            println!("  | {line}");
        }
    }
    Ok(())
}

pub fn cmd_gates(run: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let wf = load_wf()?;
    let st = state_for(&run_id, &wf)?;
    let Some(node) = current_node(&wf, &st) else {
        return Err("既に完了している".to_string());
    };
    let rc = RunCtx::load(&run_id);
    let results = eval_node_gates(&wf, node, &st, &rc);
    print_gate_lines(&results);
    Ok(())
}
