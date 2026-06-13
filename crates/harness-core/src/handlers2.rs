//! CLI サブコマンドのハンドラ（読み取り系: validate / skill / gates）と gate 評価ヘルパ。

use std::collections::HashSet;
use std::path::PathBuf;

use crate::event::{read_events, Event, EventKind};
use crate::gate::{eval_gate, GateCtx, GateResult, Question};
use crate::handlers::{load_wf, state_for};
use crate::paths;
use crate::questions::read_questions;
use crate::spec::{load_spec, Spec};
use crate::state::State;
use crate::status_view::print_gate_lines;
use crate::workflow::{current_node, load_workflow, validate, Node, Workflow};

/// run 文脈をまとめて読み込む（spec / 質問キュー / workflow スナップショット / イベント）。
pub struct RunCtx {
    pub spec: Option<Spec>,
    pub questions: Vec<Question>,
    pub snapshot: Option<String>,
    /// イベント履歴から導出した到達済みノード id（entry ＋ advance の from/to）。
    pub reached: Vec<String>,
    /// 最後の遷移/リセット以降の連続 `advance_rejected` 数（reject 残回数表示用）。
    pub reject_streak: usize,
}

/// イベント列から到達済みノード id 集合を導出する（entry ＋ advance の from/to）。
fn reached_from_events(wf: &Workflow, events: &[Event]) -> Vec<String> {
    let mut set: HashSet<String> = HashSet::new();
    set.insert(wf.meta.entry.clone());
    for ev in events {
        if let EventKind::Advance { from, to } = &ev.kind {
            set.insert(from.clone());
            set.insert(to.clone());
        }
    }
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    v
}

/// 最後の遷移/リセット以降の `advance_rejected` 連続回数。
fn reject_streak_of(events: &[Event]) -> usize {
    let mut count = 0;
    for ev in events {
        match &ev.kind {
            EventKind::Advance { .. } | EventKind::Back { .. } | EventKind::Reset => count = 0,
            EventKind::AdvanceRejected { .. } => count += 1,
            _ => {}
        }
    }
    count
}

impl RunCtx {
    pub fn load(run_id: &str) -> RunCtx {
        let spec = load_spec(&paths::spec_path()).ok();
        let questions = read_questions(run_id).unwrap_or_default();
        let snapshot = paths::workflow_snapshot_path(run_id)
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok());
        let events = read_events(run_id).unwrap_or_default();
        let reject_streak = reject_streak_of(&events);
        let reached = load_wf()
            .map(|wf| reached_from_events(&wf, &events))
            .unwrap_or_default();
        RunCtx { spec, questions, snapshot, reached, reject_streak }
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
        reached_nodes: &rc.reached,
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

/// `harness spec <F-NNN>` ── requirement の text/files/tests と紐づく acceptance/invariant を表示する。
/// spec.toml はプロジェクト単位（run 非依存）なので run 引数は使わない。
pub fn cmd_spec(requirement_id: &str, _run: Option<&str>) -> Result<(), String> {
    let spec = load_spec(&paths::spec_path())?;
    let req = spec
        .requirement
        .iter()
        .find(|r| r.id == requirement_id)
        .ok_or_else(|| format!("requirement '{requirement_id}' が spec.toml に見つからない"))?;
    println!("[{}] {}", req.id, req.text);
    if let Some(r) = &req.rationale {
        println!("rationale: {r}");
    }
    if !req.files.is_empty() {
        println!("files: {}", req.files.join(", "));
    }
    if !req.tests.is_empty() {
        println!("tests: {}", req.tests.join(", "));
    }
    let accs: Vec<_> = spec
        .acceptance
        .iter()
        .filter(|a| a.requirement == requirement_id)
        .collect();
    if !accs.is_empty() {
        println!("acceptances:");
        for a in accs {
            println!("  [{}] {}", a.id, a.text);
            println!("    test: {}", a.test);
        }
    }
    if !spec.invariant.is_empty() {
        println!("invariants:");
        for inv in &spec.invariant {
            println!("  [{}] {}", inv.id, inv.text);
        }
    }
    Ok(())
}

/// `harness artifact <name>` ── 登録済み artifact の path を表示する。
pub fn cmd_artifact(name: &str, run: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let wf = load_wf()?;
    let st = state_for(&run_id, &wf)?;
    let path = st
        .artifacts
        .get(name)
        .ok_or_else(|| format!("artifact '{name}' が未登録"))?;
    println!("{name} -> {path}");
    match std::fs::read_to_string(path) {
        Ok(body) => {
            println!("---");
            print!("{body}");
            if !body.ends_with('\n') {
                println!();
            }
        }
        Err(_) => println!("  (警告: ファイルを読めない ── 元のパスが消えた可能性)"),
    }
    Ok(())
}

/// `harness artifact-list` ── 登録済み artifact を一覧する。
pub fn cmd_artifact_list(run: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let wf = load_wf()?;
    let st = state_for(&run_id, &wf)?;
    if st.artifacts.is_empty() {
        println!("artifact なし");
        return Ok(());
    }
    for (name, path) in &st.artifacts {
        println!("{name} -> {path}");
    }
    Ok(())
}
