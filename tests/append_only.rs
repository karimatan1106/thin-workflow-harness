//! `workflow_append_only` gate の単体テスト（run-start スナップショット vs 現 workflow.toml の diff）。

use thin_workflow_harness::gate::{eval_gate, GateCtx};
use thin_workflow_harness::state::derive_state;
use thin_workflow_harness::workflow::Workflow;

/// `[meta]` ＋ node1 ＋ node2（任意で node3）の workflow.toml テキストを組み立てる。
fn wf(entry: &str, node1: &str, node2: &str, node3: Option<&str>) -> String {
    let mut s = format!(
        "[meta]\nname=\"f\"\nentry=\"{entry}\"\nmandatory_gates=[{{gate=\"traceability_closed\"}}]\n\n{node1}\n\n{node2}\n"
    );
    if let Some(n3) = node3 {
        s.push('\n');
        s.push_str(n3);
        s.push('\n');
    }
    s
}

// snapshot 時点の各ノード定義。
const N1: &str = "[[node]]\nid=\"node1\"\nskill=\"n1.md\"\ntools=[\"read\",\"write\"]\nexit_gates=[{gate=\"evidence_recorded\",args={key=\"done1\"}}]\nnext=[\"node2\"]";
const N2: &str = "[[node]]\nid=\"node2\"\nskill=\"n2.md\"\nexit_gates=[{gate=\"artifact_registered\",args={name_or_prefix=\"out\"}}]\nnext=[]";

fn snapshot() -> String {
    wf("node1", N1, N2, None)
}

fn parse(s: &str) -> Workflow {
    toml::from_str(s).expect("workflow parse")
}

fn check(cur_text: &str, reached: &[&str]) -> (bool, String) {
    let snap = snapshot();
    let cur = parse(cur_text);
    let dir = tempfile::tempdir().unwrap();
    let st = derive_state("r", &[]).finalize(1);
    let reached_v: Vec<String> = reached.iter().map(|s| s.to_string()).collect();
    let ctx = GateCtx {
        home: dir.path(),
        workflow: Some(&cur),
        workflow_snapshot: Some(&snap),
        spec: None,
        questions: &[],
        current_node: None,
        reached_nodes: &reached_v,
    };
    let r = eval_gate("workflow_append_only", &toml::Table::new(), &st, &ctx);
    (r.ok, r.note)
}

#[test]
fn unchanged_workflow_passes() {
    let (ok, note) = check(&snapshot(), &[]);
    assert!(ok, "unchanged should pass: {note}");
}

#[test]
fn adding_new_node_with_mandatory_gate_passes() {
    // node2 が未到達なので node2 → node3 配線追加は許される。新規 node3 は mandatory gate を含む。
    let n2_to_n3 = "[[node]]\nid=\"node2\"\nskill=\"n2.md\"\nexit_gates=[{gate=\"artifact_registered\",args={name_or_prefix=\"out\"}}]\nnext=[\"node3\"]";
    let n3 = "[[node]]\nid=\"node3\"\nskill=\"n3.md\"\nexit_gates=[{gate=\"traceability_closed\"},{gate=\"evidence_recorded\",args={key=\"x\"}}]\nnext=[]";
    let (ok, note) = check(&wf("node1", N1, n2_to_n3, Some(n3)), &["node1"]);
    assert!(ok, "append-only add should pass: {note}");
}

#[test]
fn new_node_missing_mandatory_gate_fails() {
    let n2_to_n3 = "[[node]]\nid=\"node2\"\nskill=\"n2.md\"\nexit_gates=[{gate=\"artifact_registered\",args={name_or_prefix=\"out\"}}]\nnext=[\"node3\"]";
    let n3 = "[[node]]\nid=\"node3\"\nskill=\"n3.md\"\nexit_gates=[{gate=\"evidence_recorded\",args={key=\"x\"}}]\nnext=[]";
    let (ok, note) = check(&wf("node1", N1, n2_to_n3, Some(n3)), &["node1"]);
    assert!(!ok, "new node without mandatory gate must fail");
    assert!(note.contains("mandatory"), "note: {note}");
}

#[test]
fn deleting_existing_node_fails() {
    let n1_no_next = "[[node]]\nid=\"node1\"\nskill=\"n1.md\"\ntools=[\"read\",\"write\"]\nexit_gates=[{gate=\"evidence_recorded\",args={key=\"done1\"}}]\nnext=[]";
    // node2 を消す ── meta だけ + node1。直接組み立てる。
    let text = format!("[meta]\nname=\"f\"\nentry=\"node1\"\n\n{n1_no_next}\n");
    let (ok, _note) = check(&text, &["node1"]);
    assert!(!ok, "deleting node2 must fail");
}

#[test]
fn weakening_exit_gate_fails() {
    let n2_empty = "[[node]]\nid=\"node2\"\nskill=\"n2.md\"\nexit_gates=[]\nnext=[]";
    let (ok, note) = check(&wf("node1", N1, n2_empty, None), &[]);
    assert!(!ok, "removing node2 exit gate must fail");
    assert!(note.contains("artifact_registered"), "note: {note}");
}

#[test]
fn widening_tools_fails() {
    let n1_wide = "[[node]]\nid=\"node1\"\nskill=\"n1.md\"\ntools=[\"read\",\"write\",\"run_command\"]\nexit_gates=[{gate=\"evidence_recorded\",args={key=\"done1\"}}]\nnext=[\"node2\"]";
    let (ok, note) = check(&wf("node1", n1_wide, N2, None), &[]);
    assert!(!ok, "adding a tool must fail");
    assert!(note.contains("run_command"), "note: {note}");
}

#[test]
fn changing_entry_fails() {
    let (ok, note) = check(&wf("node2", N1, N2, None), &[]);
    assert!(!ok, "changing entry must fail");
    assert!(note.contains("entry"), "note: {note}");
}

#[test]
fn rewiring_reached_node_fails() {
    let n1_extra = "[[node]]\nid=\"node1\"\nskill=\"n1.md\"\ntools=[\"read\",\"write\"]\nexit_gates=[{gate=\"evidence_recorded\",args={key=\"done1\"}}]\nnext=[\"node2\",\"node3\"]";
    let n3 = "[[node]]\nid=\"node3\"\nskill=\"n3.md\"\nexit_gates=[{gate=\"traceability_closed\"}]\nnext=[]";
    // node1 は到達済み ── next への追加であっても拒否。
    let (ok, _note) = check(&wf("node1", n1_extra, N2, Some(n3)), &["node1"]);
    assert!(!ok, "rewiring a reached node must fail");
}
