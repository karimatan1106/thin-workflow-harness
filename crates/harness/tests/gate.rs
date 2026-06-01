//! gate プリミティブ 6 個の単体テスト。

use thin_workflow_harness_core::event::{Event, EventKind};
use thin_workflow_harness_core::gate::{eval_gate, GateCtx};
use thin_workflow_harness_core::state::{derive_state, State};

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
    let ctx = GateCtx::minimal(dir.path());
    let st = empty_state();

    assert!(eval_gate("file_exists", &tbl(&[("path", "a.txt".into())]), &st, &ctx).ok);
    assert!(!eval_gate("file_exists", &tbl(&[("path", "nope.txt".into())]), &st, &ctx).ok);
    assert!(eval_gate("file_nonempty", &tbl(&[("path", "a.txt".into())]), &st, &ctx).ok);
    assert!(!eval_gate("file_nonempty", &tbl(&[("path", "empty.txt".into())]), &st, &ctx).ok);
}

#[test]
fn gate_cmd_exit_0() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = GateCtx::minimal(dir.path());
    let st = empty_state();
    let ok_cmd = if cfg!(windows) { "exit 0" } else { "true" };
    assert!(eval_gate("cmd_exit_0", &tbl(&[("cmd", ok_cmd.into())]), &st, &ctx).ok);
    assert!(!eval_gate("cmd_exit_0", &tbl(&[("cmd", "exit 3".into())]), &st, &ctx).ok);
}

#[test]
fn gate_evidence_and_json_has() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = GateCtx::minimal(dir.path());
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
fn gate_json_nonempty_and_json_in() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = GateCtx::minimal(dir.path());
    // master_design_update を模した evidence: updated/中身あり と noop/空配列 の両ケース。
    let events = vec![ev(EventKind::GateEvidence {
        gate: "master_design_update".into(),
        data: serde_json::json!({
            "verdict": "updated",
            "rationale": "WS coalesce 方針変更を 02-blocks に反映",
            "architecture_sections_changed": ["02-blocks"],
            "empty_arr": [],
            "blank": "  "
        }),
    })];
    let st = derive_state("r", &events).finalize(1);

    // json_nonempty: 実体ある配列/文字列は ok、空配列/空白文字列は fail。
    let sections = tbl(&[
        ("evidence_key", "master_design_update".into()),
        ("json_path", "architecture_sections_changed".into()),
    ]);
    assert!(eval_gate("json_nonempty", &sections, &st, &ctx).ok);
    let rationale = tbl(&[
        ("evidence_key", "master_design_update".into()),
        ("json_path", "rationale".into()),
    ]);
    assert!(eval_gate("json_nonempty", &rationale, &st, &ctx).ok);
    let empty_arr = tbl(&[
        ("evidence_key", "master_design_update".into()),
        ("json_path", "empty_arr".into()),
    ]);
    assert!(!eval_gate("json_nonempty", &empty_arr, &st, &ctx).ok, "空配列は fail");
    let blank = tbl(&[
        ("evidence_key", "master_design_update".into()),
        ("json_path", "blank".into()),
    ]);
    assert!(!eval_gate("json_nonempty", &blank, &st, &ctx).ok, "空白のみ文字列は fail");
    let missing = tbl(&[
        ("evidence_key", "master_design_update".into()),
        ("json_path", "nope".into()),
    ]);
    assert!(!eval_gate("json_nonempty", &missing, &st, &ctx).ok, "存在しない path は fail");

    // json_in: 許可値内は ok、逃げ値(no_change)は fail。
    let allowed = tbl(&[
        ("evidence_key", "master_design_update".into()),
        ("json_path", "verdict".into()),
        ("one_of", "updated,noop".into()),
    ]);
    assert!(eval_gate("json_in", &allowed, &st, &ctx).ok);

    let no_change_events = vec![ev(EventKind::GateEvidence {
        gate: "master_design_update".into(),
        data: serde_json::json!({"verdict": "no_change"}),
    })];
    let st2 = derive_state("r", &no_change_events).finalize(1);
    assert!(
        !eval_gate("json_in", &allowed, &st2, &ctx).ok,
        "no_change は許可値外なので fail"
    );
}

#[test]
fn gate_artifact_registered_prefix_and_existence() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("impl_a.rs");
    std::fs::write(&f, b"x").unwrap();
    let ctx = GateCtx::minimal(dir.path());
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
    let ctx = GateCtx::minimal(dir.path());
    let r = eval_gate("does_not_exist", &toml::Table::new(), &empty_state(), &ctx);
    assert!(!r.ok);
    assert!(r.note.contains("unknown gate"));
}
