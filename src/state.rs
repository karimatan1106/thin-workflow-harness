//! State 型と `derive_state`（イベント列の純粋 fold）。
//!
//! `Reset` が来たら「それ以降のイベントだけ」で再構築する（`run_id`/`intent` は最初の `Start` から保持）。

use std::collections::BTreeMap;

use crate::event::{Event, EventKind};

/// 表示用の履歴項目（再構築には使わない）。
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryItem {
    pub kind: String,
    pub detail: String,
}

/// run の派生状態（イベントログから fold したスナップショット）。
#[derive(Debug, Clone)]
pub struct State {
    pub run_id: String,
    pub intent: String,
    /// 現在ノードのインデックス（0 始まり）。`>= node_count` のとき done。
    pub phase_index: usize,
    pub artifacts: BTreeMap<String, String>,
    pub gate_evidence: BTreeMap<String, serde_json::Value>,
    /// `phase_index >= node_count` のとき true（`finalize` で確定）。
    pub done: bool,
    /// `abandon` イベントが来たら true（terminal）。
    pub abandoned: bool,
    pub history: Vec<HistoryItem>,
}

impl State {
    fn empty(run_id: &str) -> Self {
        State {
            run_id: run_id.to_string(),
            intent: String::new(),
            phase_index: 0,
            artifacts: BTreeMap::new(),
            gate_evidence: BTreeMap::new(),
            done: false,
            abandoned: false,
            history: Vec::new(),
        }
    }

    /// node_count を渡して `done` を確定させる。
    pub fn finalize(mut self, node_count: usize) -> Self {
        self.done = self.phase_index >= node_count;
        self
    }
}

fn first_intent(events: &[Event]) -> String {
    events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::Start { intent } => Some(intent.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// イベント列から State を導出する純粋 fold（workflow を見ない後方互換版）。
/// `Reset` 以降のスライスで再構築するが、`run_id`/`intent` は保持。
pub fn derive_state(run_id: &str, events: &[Event]) -> State {
    derive_state_inner(run_id, events, None)
}

/// `derive_state` の workflow 解決版 ── Advance の `to` が wf 内のノード id ならその
/// index へジャンプ（fork→jn の非自然遷移用）、未知 id なら `phase_index += 1`。
pub fn derive_state_resolving(run_id: &str, events: &[Event], wf: &crate::workflow::Workflow) -> State {
    derive_state_inner(run_id, events, Some(wf))
}

fn derive_state_inner(run_id: &str, events: &[Event], wf: Option<&crate::workflow::Workflow>) -> State {
    let last_reset = events
        .iter()
        .rposition(|e| matches!(e.kind, EventKind::Reset));
    let slice: &[Event] = match last_reset {
        Some(i) => &events[i + 1..],
        None => events,
    };

    let mut st = State::empty(run_id);
    st.intent = first_intent(events);

    for ev in slice {
        match &ev.kind {
            EventKind::Start { intent } => {
                st.intent = intent.clone();
            }
            EventKind::Advance { from, to } => {
                match wf.and_then(|w| w.index_of(to)) {
                    Some(idx) => st.phase_index = idx,
                    None => st.phase_index += 1,
                }
                st.history.push(HistoryItem {
                    kind: "advance".into(),
                    detail: format!("{from} -> {to}"),
                });
            }
            EventKind::AdvanceRejected { failed_gates } => {
                let names: Vec<&str> = failed_gates.iter().map(|g| g.gate.as_str()).collect();
                st.history.push(HistoryItem {
                    kind: "advance_rejected".into(),
                    detail: names.join(", "),
                });
            }
            EventKind::Back { reason } => {
                st.phase_index = st.phase_index.saturating_sub(1);
                st.history.push(HistoryItem {
                    kind: "back".into(),
                    detail: reason.clone(),
                });
            }
            EventKind::Artifact { name, path, tag } => {
                st.artifacts.insert(name.clone(), path.clone());
                let tagdetail = tag.as_deref().map(|t| format!(" #{t}")).unwrap_or_default();
                st.history.push(HistoryItem {
                    kind: "artifact".into(),
                    detail: format!("{name} ({path}{tagdetail})"),
                });
            }
            EventKind::GateEvidence { gate, data } => {
                st.gate_evidence.insert(gate.clone(), data.clone());
                st.history.push(HistoryItem {
                    kind: "gate_evidence".into(),
                    detail: gate.clone(),
                });
            }
            EventKind::Reset => {
                st = State::empty(run_id);
                st.intent = first_intent(events);
            }
            EventKind::QuestionQueued { question_id, kind, .. } => {
                st.history.push(HistoryItem {
                    kind: "question_queued".into(),
                    detail: format!("{question_id} ({kind})"),
                });
            }
            EventKind::HumanAnswer { question_id, answer } => {
                st.history.push(HistoryItem {
                    kind: "human_answer".into(),
                    detail: format!("{question_id} -> {answer}"),
                });
            }
            EventKind::Abandon { reason } => {
                st.abandoned = true;
                st.history.push(HistoryItem { kind: "abandon".into(), detail: reason.clone() });
            }
            EventKind::BranchForked { branch_ids } => {
                st.history.push(HistoryItem {
                    kind: "branch_forked".into(),
                    detail: branch_ids.join(","),
                });
            }
            EventKind::BranchJoined { branch_ids, status, failures } => {
                // 成功時のみ branch sub-log を読んで artifact/gate_evidence をメインに fold-in。
                if status == "success" {
                    for bid in branch_ids {
                        if let Ok(sub) = crate::event::read_branch_events(run_id, bid) {
                            for sev in &sub {
                                match &sev.kind {
                                    EventKind::Artifact { name, path, tag } => {
                                        st.artifacts.insert(name.clone(), path.clone());
                                        let td = tag.as_deref().map(|t| format!(" #{t}")).unwrap_or_default();
                                        st.history.push(HistoryItem {
                                            kind: "artifact".into(),
                                            detail: format!("[{bid}] {name} ({path}{td})"),
                                        });
                                    }
                                    EventKind::GateEvidence { gate, data } => {
                                        st.gate_evidence.insert(gate.clone(), data.clone());
                                        st.history.push(HistoryItem {
                                            kind: "gate_evidence".into(),
                                            detail: format!("[{bid}] {gate}"),
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                let detail = match failures {
                    Some(fs) if !fs.is_empty() => format!("{} status={} failures={}", branch_ids.join(","), status, fs.join(";")),
                    _ => format!("{} status={}", branch_ids.join(","), status),
                };
                st.history.push(HistoryItem { kind: "branch_joined".into(), detail });
            }
        }
    }
    st
}
