//! `branch_apply.rs` のユニットテスト ── pure な ApplyResult 経路だけ検証。
//! 実 API は呼ばない（足場テスト）。
//!
//! 親ファイル側で `#[path = "branch_apply_tests.rs"] mod tests;` で取り込まれる。

use super::*;
use crate::workflow::Node;
use std::path::PathBuf;
use tempfile::TempDir;

fn make_intc(cwd: PathBuf) -> Interceptor {
    let node = Node {
        id: "b1".into(),
        r#type: None,
        skill: None,
        serves: vec![],
        exit_gates: vec![],
        next: vec![],
        branches: vec![],
        wait: vec![],
        on_reject: None,
        tools: vec![],
        can_append: false,
        context: None,
        artifact_tags: vec![],
        model: None,
        budget: None,
        cmd_allowlist: vec![],
        network: false,
        files: vec![],
    };
    Interceptor::for_node(&node, None, cwd)
}

#[test]
fn create_file_writes_under_cwd() {
    let tmp = TempDir::new().unwrap();
    let intc = make_intc(tmp.path().to_path_buf());
    let r = apply_branch_action(
        "run-x",
        "b1",
        &intc,
        WorkerAction::CreateFile { path: "out.txt".into(), content: "hi".into() },
    );
    assert!(!r.is_error, "{}", r.content);
    assert!(r.terminal.is_none());
    assert_eq!(std::fs::read_to_string(tmp.path().join("out.txt")).unwrap(), "hi");
}

#[test]
fn request_transition_returns_terminal_transitioned() {
    let tmp = TempDir::new().unwrap();
    let intc = make_intc(tmp.path().to_path_buf());
    let r = apply_branch_action("run-x", "b1", &intc, WorkerAction::RequestTransition);
    assert!(!r.is_error);
    assert!(matches!(r.terminal, Some(Outcome::Transitioned)));
}

#[test]
fn stuck_returns_terminal_stuck() {
    let tmp = TempDir::new().unwrap();
    let intc = make_intc(tmp.path().to_path_buf());
    let r = apply_branch_action(
        "run-x",
        "b1",
        &intc,
        WorkerAction::Stuck { reason: "詰まり".into() },
    );
    assert!(!r.is_error);
    match r.terminal {
        Some(Outcome::Stuck(reason)) => assert_eq!(reason, "詰まり"),
        other => panic!("expected Stuck, got {other:?}"),
    }
}

#[test]
fn run_command_is_unsupported_in_branch() {
    let tmp = TempDir::new().unwrap();
    let intc = make_intc(tmp.path().to_path_buf());
    let r = apply_branch_action(
        "run-x",
        "b1",
        &intc,
        WorkerAction::RunCommand { cmd: "ls".into() },
    );
    assert!(r.is_error);
    assert!(r.content.contains("run_command 未対応"));
}
