//! 各サブコマンドの実装。

use std::fs;
use std::path::Path;

use chrono::Utc;
use serde_json::Value;

use crate::gates::eval_gate;
use crate::paths::{resolve_run_id, skill_path, state_dir};
use crate::phases::{current_phase, PHASES};
use crate::state::{append_event, derive_state, read_events, EventKind, FailedGate, State};

fn load_state(run: Option<String>) -> Result<State, String> {
    let run_id = resolve_run_id(run.as_deref())?;
    let events = read_events(&run_id)?;
    Ok(derive_state(&run_id, &events))
}

/// 一意な run_id を作る（YYYYMMDD_HHMMSS、同秒衝突は _b, _c ...）。
fn new_run_id() -> Result<String, String> {
    let base = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let dir = state_dir()?;
    if !dir.join(format!("{base}.jsonl")).exists() {
        return Ok(base);
    }
    for suffix in b'b'..=b'z' {
        let cand = format!("{base}_{}", suffix as char);
        if !dir.join(format!("{cand}.jsonl")).exists() {
            return Ok(cand);
        }
    }
    Err("run_id 採番に失敗しました（同秒で多数の run）".into())
}

pub fn cmd_start(intent: String) -> Result<(), String> {
    if intent.trim().is_empty() {
        return Err("intent が空です".into());
    }
    let run_id = new_run_id()?;
    append_event(&run_id, EventKind::Start { intent: intent.clone() })?;
    println!("run 開始: {run_id}");
    println!("ヒント: HARNESS_RUN={run_id} を環境変数にすると以後 --run を省略できます。");
    println!();
    print_status(&load_state(Some(run_id))?);
    Ok(())
}

fn print_status(state: &State) {
    println!("run_id : {}", state.run_id);
    println!("intent : {}", state.intent);
    if state.done() {
        println!("✔ 完了（全フェーズ通過）");
        return;
    }
    let phase = current_phase(state).expect("not done なのでフェーズあり");
    println!("フェーズ: {}/{} {}", state.phase_index + 1, PHASES.len(), phase.name);
    println!("skill  : {}（これを読んでから作業）", skill_path(phase.skill).display());
    println!("出口 gate:");
    for g in phase.exit_gates {
        let r = eval_gate(g, state);
        if r.ok {
            println!("  [PASS] {g}");
        } else {
            println!("  [FAIL] {g} — {}", r.note);
        }
    }
    println!("成果物:");
    if state.artifacts.is_empty() {
        println!("  (なし)");
    } else {
        for (k, v) in &state.artifacts {
            println!("  {k} -> {v}");
        }
    }
    println!("gate 根拠:");
    if state.gate_evidence.is_empty() {
        println!("  (なし)");
    } else {
        for k in state.gate_evidence.keys() {
            println!("  {k}");
        }
    }
}

pub fn cmd_status(run: Option<String>) -> Result<(), String> {
    print_status(&load_state(run)?);
    Ok(())
}

pub fn cmd_advance(run: Option<String>) -> Result<(), String> {
    let state = load_state(run)?;
    if state.done() {
        return Err("既に完了しています（advance 不要）".into());
    }
    let phase = current_phase(&state).expect("not done");
    let mut failed: Vec<FailedGate> = Vec::new();
    for g in phase.exit_gates {
        let r = eval_gate(g, &state);
        if !r.ok {
            failed.push(FailedGate { gate: (*g).to_string(), reason: r.note });
        }
    }
    if failed.is_empty() {
        let from = phase.name.to_string();
        let to = PHASES.get(state.phase_index + 1).map(|p| p.name.to_string()).unwrap_or_else(|| "(done)".into());
        append_event(&state.run_id, EventKind::Advance { from: from.clone(), to: to.clone() })?;
        println!("advance: {from} -> {to}");
        println!();
        print_status(&load_state(Some(state.run_id))?);
        Ok(())
    } else {
        let lines: Vec<String> = failed.iter().map(|f| format!("{}: {}", f.gate, f.reason)).collect();
        append_event(&state.run_id, EventKind::AdvanceRejected { failed_gates: failed })?;
        println!("advance 却下。未達 gate:");
        for l in &lines {
            println!("  [FAIL] {l}");
        }
        Err(format!("advance 却下: {}", lines.join("; ")))
    }
}

pub fn cmd_back(reason: String, run: Option<String>) -> Result<(), String> {
    let state = load_state(run)?;
    if state.phase_index == 0 {
        return Err("既に最初のフェーズです（back 不可）".into());
    }
    append_event(&state.run_id, EventKind::Back { reason: reason.clone() })?;
    println!("back: {reason}");
    println!();
    print_status(&load_state(Some(state.run_id))?);
    Ok(())
}

pub fn cmd_record_artifact(name: String, path_str: String, run: Option<String>) -> Result<(), String> {
    let state = load_state(run)?;
    let p = Path::new(&path_str);
    let abs = fs::canonicalize(p).map_err(|e| format!("パス解決失敗 {path_str}: {e}"))?;
    if !abs.is_file() {
        return Err(format!("ファイルではありません: {}", abs.display()));
    }
    let abs_str = abs.to_string_lossy().to_string();
    append_event(&state.run_id, EventKind::Artifact { name: name.clone(), path: abs_str.clone() })?;
    println!("{name} 登録: {abs_str}");
    Ok(())
}

pub fn cmd_report_gate(gate: String, json_arg: String, run: Option<String>) -> Result<(), String> {
    let state = load_state(run)?;
    let raw = if let Some(rest) = json_arg.strip_prefix('@') {
        fs::read_to_string(rest).map_err(|e| format!("JSON ファイル読取失敗 {rest}: {e}"))?
    } else {
        json_arg.clone()
    };
    let data: Value = serde_json::from_str(raw.trim()).map_err(|e| format!("JSON 解析失敗: {e}"))?;
    append_event(&state.run_id, EventKind::GateEvidence { gate: gate.clone(), data })?;
    println!("{gate} の根拠を記録");
    Ok(())
}

pub fn cmd_reset(run: Option<String>, yes: bool) -> Result<(), String> {
    if !yes {
        return Err("確認のため --yes を付けてください".into());
    }
    let state = load_state(run)?;
    append_event(&state.run_id, EventKind::Reset)?;
    println!("reset 実行: {}", state.run_id);
    println!();
    print_status(&load_state(Some(state.run_id))?);
    Ok(())
}
