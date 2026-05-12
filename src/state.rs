//! append-only イベントログ (jsonl) と決定論的なリプレイ。

use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::paths::event_log_path;
use crate::phases::PHASES;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FailedGate {
    pub gate: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    Start { intent: String },
    Advance { from: String, to: String },
    AdvanceRejected { failed_gates: Vec<FailedGate> },
    Back { reason: String },
    Artifact { name: String, path: String },
    GateEvidence { gate: String, data: Value },
    Reset,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    pub ts: String,
    #[serde(flatten)]
    pub kind: EventKind,
}

fn now_ts() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// ts を付けて 1 行追記する。
pub fn append_event(run_id: &str, kind: EventKind) -> Result<(), String> {
    let event = Event { ts: now_ts(), kind };
    let line = serde_json::to_string(&event).map_err(|e| format!("イベント直列化失敗: {e}"))?;
    let path = event_log_path(run_id)?;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("イベントログ open 失敗 {}: {e}", path.display()))?;
    writeln!(f, "{line}").map_err(|e| format!("イベントログ書込失敗: {e}"))?;
    Ok(())
}

/// イベントを全件読む。ファイル無しなら空。壊れた行はエラー。
pub fn read_events(run_id: &str) -> Result<Vec<Event>, String> {
    let path = event_log_path(run_id)?;
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("イベントログ読取失敗 {}: {e}", path.display())),
    };
    let mut events = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let ev: Event = serde_json::from_str(trimmed)
            .map_err(|e| format!("イベントログ {} の {} 行目が壊れています: {e}", path.display(), i + 1))?;
        events.push(ev);
    }
    Ok(events)
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // 表示・デバッグ用。フィールドは Debug 経由で参照される。
pub struct HistoryItem {
    pub kind: String,
    pub detail: String,
}

#[derive(Debug)]
pub struct State {
    pub run_id: String,
    pub intent: String,
    pub phase_index: usize,
    pub artifacts: BTreeMap<String, String>,
    pub gate_evidence: BTreeMap<String, Value>,
    pub history: Vec<HistoryItem>,
}

impl State {
    pub fn done(&self) -> bool {
        self.phase_index >= PHASES.len()
    }
}

/// イベントをリプレイして状態を導出する。
pub fn derive_state(run_id: &str, events: &[Event]) -> State {
    // run_id / intent は最初の Start から取る。
    let mut intent = String::new();
    for ev in events {
        if let EventKind::Start { intent: i } = &ev.kind {
            intent = i.clone();
            break;
        }
    }

    // Reset があれば最後の Reset 以降のイベントだけで再構築する。
    let mut start = 0usize;
    for (idx, ev) in events.iter().enumerate() {
        if matches!(ev.kind, EventKind::Reset) {
            start = idx + 1;
        }
    }

    let mut state = State {
        run_id: run_id.to_string(),
        intent,
        phase_index: 0,
        artifacts: BTreeMap::new(),
        gate_evidence: BTreeMap::new(),
        history: Vec::new(),
    };

    for ev in &events[start..] {
        match &ev.kind {
            EventKind::Start { intent } => {
                if state.intent.is_empty() {
                    state.intent = intent.clone();
                }
                state.history.push(HistoryItem { kind: "start".into(), detail: intent.clone() });
            }
            EventKind::Advance { from, to } => {
                state.phase_index += 1;
                state.history.push(HistoryItem { kind: "advance".into(), detail: format!("{from} -> {to}") });
            }
            EventKind::AdvanceRejected { failed_gates } => {
                let detail = failed_gates
                    .iter()
                    .map(|g| format!("{}: {}", g.gate, g.reason))
                    .collect::<Vec<_>>()
                    .join("; ");
                state.history.push(HistoryItem { kind: "advance_rejected".into(), detail });
            }
            EventKind::Back { reason } => {
                state.phase_index = state.phase_index.saturating_sub(1);
                state.history.push(HistoryItem { kind: "back".into(), detail: reason.clone() });
            }
            EventKind::Artifact { name, path } => {
                state.artifacts.insert(name.clone(), path.clone());
                state.history.push(HistoryItem { kind: "artifact".into(), detail: format!("{name} = {path}") });
            }
            EventKind::GateEvidence { gate, data } => {
                state.gate_evidence.insert(gate.clone(), data.clone());
                state.history.push(HistoryItem { kind: "gate_evidence".into(), detail: gate.clone() });
            }
            EventKind::Reset => {}
        }
    }

    state
}
