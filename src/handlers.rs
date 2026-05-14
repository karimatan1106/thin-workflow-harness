//! CLI サブコマンドのハンドラ（run の状態を変える系 + status）。

use chrono::{SecondsFormat, Utc};

use crate::event::{append_event, read_events, EventKind};
use crate::handlers2::RunCtx;
use crate::spec::load_spec;
use crate::state::{derive_state_resolving, State};
use crate::status_view::print_status;
use crate::workflow::{load_workflow, validate, Workflow};
use crate::paths;

pub(crate) fn load_wf() -> Result<Workflow, String> {
    load_workflow(&paths::workflow_path())
}

pub(crate) fn state_for(run_id: &str, wf: &Workflow) -> Result<State, String> {
    let events = read_events(run_id)?;
    Ok(derive_state_resolving(run_id, &events, wf).finalize(wf.nodes().len()))
}

/// abandon 済みなら遷移系コマンドを拒否する。
pub(crate) fn ensure_active(st: &State) -> Result<(), String> {
    if st.abandoned {
        return Err("この run は放棄済み（abandon）── 遷移できない".to_string());
    }
    Ok(())
}

pub(crate) fn show(wf: &Workflow, run_id: &str) -> Result<(), String> {
    let st = state_for(run_id, wf)?;
    let rc = RunCtx::load(run_id);
    print_status(wf, &st, &rc);
    Ok(())
}

fn new_run_id() -> Result<String, String> {
    let base = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let dir = paths::state_dir()?;
    if !dir.join(format!("{base}.jsonl")).exists() {
        return Ok(base);
    }
    for suffix in 'b'..='z' {
        let cand = format!("{base}_{suffix}");
        if !dir.join(format!("{cand}.jsonl")).exists() {
            return Ok(cand);
        }
    }
    Err("run id の衝突を解消できない".to_string())
}

pub fn cmd_start(intent: &str, worktree: Option<&str>) -> Result<(), String> {
    let wf = load_wf()?;
    let errs = validate(&wf, load_spec(&paths::spec_path()).ok().as_ref());
    if !errs.is_empty() {
        return Err(format!("workflow.toml/spec.toml に問題:\n  - {}", errs.join("\n  - ")));
    }
    if let Some(wt) = worktree {
        // worktree モードは skeleton では scaffold ── ここでは記録だけ（実体は後フェーズ）。
        println!("(注) --worktree {wt} は scaffold ── 隔離の実体は未実装");
    }
    let run_id = new_run_id()?;
    // workflow.toml スナップショットをサイドカーに保存（workflow_append_only 用）
    if let Ok(text) = std::fs::read_to_string(paths::workflow_path()) {
        let snap = paths::workflow_snapshot_path(&run_id)?;
        std::fs::write(&snap, text).map_err(|e| format!("snapshot 書込失敗 {}: {e}", snap.display()))?;
    }
    append_event(&run_id, EventKind::Start { intent: intent.to_string() })
        .map_err(|e| format!("start イベント書込失敗: {e}"))?;
    println!("run {run_id} を開始 ({})", Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
    show(&wf, &run_id)
}

pub fn cmd_status(run: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let wf = load_wf()?;
    show(&wf, &run_id)
}

pub fn cmd_back(reason: &str, run: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let wf = load_wf()?;
    let st = state_for(&run_id, &wf)?;
    ensure_active(&st)?;
    if st.phase_index == 0 {
        return Err("先頭ノードなので戻れない".to_string());
    }
    append_event(&run_id, EventKind::Back { reason: reason.to_string() })
        .map_err(|e| format!("back 書込失敗: {e}"))?;
    println!("back: {reason}");
    show(&wf, &run_id)
}

pub fn cmd_record_artifact(
    name: &str,
    path: &str,
    tag: Option<&str>,
    run: Option<&str>,
) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let cwd = std::env::current_dir().map_err(|e| format!("cwd 取得失敗: {e}"))?;
    let canon = cwd
        .join(path)
        .canonicalize()
        .map_err(|_| format!("artifact ファイルが無い: {path}"))?;
    append_event(
        &run_id,
        EventKind::Artifact {
            name: name.to_string(),
            path: canon.to_string_lossy().to_string(),
            tag: tag.map(|s| s.to_string()),
        },
    )
    .map_err(|e| format!("artifact 書込失敗: {e}"))?;
    println!("{name} 登録 ({})", canon.display());
    Ok(())
}

pub fn cmd_report_evidence(gate: &str, json: &str, run: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let raw = if let Some(file) = json.strip_prefix('@') {
        std::fs::read_to_string(file).map_err(|e| format!("evidence ファイル読取失敗 {file}: {e}"))?
    } else {
        json.to_string()
    };
    let data: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("evidence JSON パース失敗: {e}"))?;
    append_event(&run_id, EventKind::GateEvidence { gate: gate.to_string(), data })
        .map_err(|e| format!("gate_evidence 書込失敗: {e}"))?;
    println!("{gate} の根拠を記録");
    Ok(())
}

pub fn cmd_reset(run: Option<&str>, yes: bool) -> Result<(), String> {
    if !yes {
        return Err("確認のため --yes を付けてください".to_string());
    }
    let run_id = paths::resolve_run_id(run)?;
    append_event(&run_id, EventKind::Reset).map_err(|e| format!("reset 書込失敗: {e}"))?;
    println!("run {run_id} をリセット");
    let wf = load_wf()?;
    show(&wf, &run_id)
}
