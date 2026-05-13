//! gate プリミティブ 6 個の単体テスト。

use thin_workflow_harness::event::{Event, EventKind};
use thin_workflow_harness::gate::{eval_gate, GateCtx};
use thin_workflow_harness::state::{derive_state, State};

fn ev(kind: EventKind) -> Event {
    Event { ts: "2026-01-01T00:00:00Z".to_string(), kind }
}

fn empty_state() -> State {
    derive_state("r", &[]).finalize(1)
}

fn tbl(pairs: &[(&str, toml::Value)]) -> toml::Table {
    let mut t = toml::Table::new();
    for (k, v) in pairs {
        t.insert((*k).to_string(), v.clone());
    }
    t
}

#[test]
fn gate_file_exists_and_nonempty() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), b"hello").unwrap();
    std::fs::write(dir.path().join("empty.txt"), b"").unwrap();
    let ctx = GateCtx { home: dir.path() };
    let st = empty_state();

    assert!(eval_gate("file_exists", &tbl(&[("path", "a.txt".into())]), &st, &ctx).ok);
    assert!(!eval_gate("file_exists", &tbl(&[("path", "nope.txt".into())]), &st, &ctx).ok);
    assert!(eval_gate("file_nonempty", &tbl(&[("path", "a.txt".into())]), &st, &ctx).ok);
    assert!(!eval_gate("file_nonempty", &tbl(&[("path", "empty.txt".into())]), &st, &ctx).ok);
}

#[test]
fn gate_cmd_exit_0() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = GateCtx { home: dir.path() };
    let st = empty_state();
    let ok_cmd = if cfg!(windows) { "exit 0" } else { "true" };
    assert!(eval_gate("cmd_exit_0", &tbl(&[("cmd", ok_cmd.into())]), &st, &ctx).ok);
    assert!(!eval_gate("cmd_exit_0", &tbl(&[("cmd", "exit 3".into())]), &st, &ctx).ok);
}

#[test]
fn gate_evidence_and_json_has() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = GateCtx { home: dir.path() };
    let events = vec![ev(EventKind::GateEvidence {
        gate: "review".into(),
        data: serde_json::json!({"verdict": "approved", "nested": {"x": 1}}),
    })];
    let st = derive_state("r", &events).finalize(1);

    assert!(eval_gate("evidence_recorded", &tbl(&[("key", "review".into())]), &st, &ctx).ok);
    assert!(!eval_gate("evidence_recorded", &tbl(&[("key", "missing".into())]), &st, &ctx).ok);

    let approved = tbl(&[
        ("evidence_key", "review".into()),
        ("json_path", "verdict".into()),
        ("eq", "approved".into()),
    ]);
    assert!(eval_gate("json_has", &approved, &st, &ctx).ok);

    let rejected = tbl(&[
        ("evidence_key", "review".into()),
        ("json_path", "verdict".into()),
        ("eq", "rejected".into()),
    ]);
    assert!(!eval_gate("json_has", &rejected, &st, &ctx).ok);

    let nested = tbl(&[("evidence_key", "review".into()), ("json_path", "nested.x".into())]);
    assert!(eval_gate("json_has", &nested, &st, &ctx).ok);
}

#[test]
fn gate_artifact_registered_prefix_and_existence() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("impl_a.rs");
    std::fs::write(&f, b"x").unwrap();
    let ctx = GateCtx { home: dir.path() };
    let events = vec![
        ev(EventKind::Start { intent: "i".into() }),
        ev(EventKind::Artifact {
            name: "impl:a".into(),
            path: f.to_string_lossy().to_string(),
            tag: None,
        }),
    ];
    let st = derive_state("r", &events).finalize(1);

    assert!(eval_gate("artifact_registered", &tbl(&[("name_or_prefix", "impl:".into())]), &st, &ctx).ok);
    assert!(!eval_gate("artifact_registered", &tbl(&[("name_or_prefix", "plan".into())]), &st, &ctx).ok);
}

#[test]
fn gate_unknown_name() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = GateCtx { home: dir.path() };
    let r = eval_gate("does_not_exist", &toml::Table::new(), &empty_state(), &ctx);
    assert!(!r.ok);
    assert!(r.note.contains("unknown gate"));
}
