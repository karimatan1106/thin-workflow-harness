//! derive_state（純粋 fold）と workflow/spec ロード・validate の単体テスト。

use std::path::Path;

use thin_workflow_harness_core::event::{Event, EventKind, FailedGate};
use thin_workflow_harness_core::state::derive_state;

fn ev(kind: EventKind) -> Event {
    Event { ts: "2026-01-01T00:00:00Z".to_string(), kind }
}

#[test]
fn derive_state_basic_fold() {
    let events = vec![
        ev(EventKind::Start { intent: "do x".into() }),
        ev(EventKind::Advance { from: "a".into(), to: "b".into() }),
        ev(EventKind::Artifact { name: "out".into(), path: "/tmp/out".into(), tag: None }),
        ev(EventKind::GateEvidence { gate: "k".into(), data: serde_json::json!({"v":1}) }),
    ];
    let st = derive_state("r1", &events).finalize(3);
    assert_eq!(st.run_id, "r1");
    assert_eq!(st.intent, "do x");
    assert_eq!(st.phase_index, 1);
    assert_eq!(st.artifacts.get("out").map(String::as_str), Some("/tmp/out"));
    assert!(st.gate_evidence.contains_key("k"));
    assert!(!st.done);
}

#[test]
fn derive_state_back_saturates() {
    let events = vec![
        ev(EventKind::Start { intent: "i".into() }),
        ev(EventKind::Back { reason: "oops".into() }),
        ev(EventKind::Back { reason: "again".into() }),
    ];
    let st = derive_state("r", &events).finalize(2);
    assert_eq!(st.phase_index, 0);
}

#[test]
fn derive_state_reset_rebuilds_from_after() {
    let events = vec![
        ev(EventKind::Start { intent: "keep me".into() }),
        ev(EventKind::Advance { from: "a".into(), to: "b".into() }),
        ev(EventKind::Advance { from: "b".into(), to: "c".into() }),
        ev(EventKind::Artifact { name: "x".into(), path: "/p".into(), tag: None }),
        ev(EventKind::Reset),
        ev(EventKind::Advance { from: "a".into(), to: "b".into() }),
    ];
    let st = derive_state("r", &events).finalize(3);
    assert_eq!(st.intent, "keep me");
    assert_eq!(st.phase_index, 1);
    assert!(st.artifacts.is_empty());
}

#[test]
fn derive_state_done_flag() {
    let events = vec![
        ev(EventKind::Start { intent: "i".into() }),
        ev(EventKind::Advance { from: "a".into(), to: "b".into() }),
        ev(EventKind::Advance { from: "b".into(), to: "(done)".into() }),
    ];
    let st = derive_state("r", &events).finalize(2);
    assert!(st.done);
}

#[test]
fn advance_rejected_recorded_in_history_only() {
    let events = vec![
        ev(EventKind::Start { intent: "i".into() }),
        ev(EventKind::AdvanceRejected {
            failed_gates: vec![FailedGate { gate: "g".into(), reason: "no".into() }],
        }),
    ];
    let st = derive_state("r", &events).finalize(1);
    assert_eq!(st.phase_index, 0);
    assert!(st.history.iter().any(|h| h.kind == "advance_rejected"));
}

#[test]
fn load_workflow_and_spec_fixture() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let wf = thin_workflow_harness_core::workflow::load_workflow(&base.join("workflow.toml")).unwrap();
    assert_eq!(wf.meta.entry, "node1");
    assert_eq!(wf.nodes().len(), 2);
    let errs = thin_workflow_harness_core::workflow::validate(&wf, None);
    assert!(errs.is_empty(), "{errs:?}");

    let spec = thin_workflow_harness_core::spec::load_spec(&base.join("spec.toml")).unwrap();
    assert_eq!(spec.requirement.len(), 1);
    assert_eq!(spec.acceptance.len(), 1);
}

#[test]
fn validate_catches_bad_entry_and_unknown_gate() {
    let toml_src = r#"
[meta]
name = "bad"
entry = "ghost"

[[node]]
id = "n1"
skill = "n1.md"
exit_gates = [ { gate = "no_such_gate", args = {} } ]
next = ["nowhere"]
"#;
    let wf: thin_workflow_harness_core::workflow::Workflow = toml::from_str(toml_src).unwrap();
    let errs = thin_workflow_harness_core::workflow::validate(&wf, None);
    assert!(errs.iter().any(|e| e.contains("entry")));
    assert!(errs.iter().any(|e| e.contains("未知の gate")));
    assert!(errs.iter().any(|e| e.contains("nowhere")));
}
