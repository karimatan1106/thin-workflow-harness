//! tool-call インターセプタの単体テスト ── blast radius / cmd_allowlist / network 判定。

use thin_workflow_harness_core::runtime::interceptor::{Interceptor, Verdict};
use thin_workflow_harness_core::workflow::Workflow;

fn node_from(toml_text: &str) -> thin_workflow_harness_core::workflow::Node {
    let wf: Workflow = toml::from_str(toml_text).expect("workflow parse");
    wf.node.into_iter().next().expect("one node")
}

const BR_NODE: &str = r#"
[meta]
name = "f"
entry = "n"

[[node]]
id = "n"
skill = "n.md"
files = ["src/a.rs", "src/sub/**/*.rs"]
cmd_allowlist = ["cargo test", "cargo build *"]
next = []
"#;

const OPEN_NODE: &str = r#"
[meta]
name = "f"
entry = "n"

[[node]]
id = "n"
skill = "n.md"
network = true
next = []
"#;

#[test]
fn write_inside_blast_radius_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let intc = Interceptor::for_node(&node_from(BR_NODE), None, dir.path().to_path_buf());
    assert_eq!(intc.check_write(&dir.path().join("src/a.rs")), Verdict::Allow);
    assert_eq!(intc.check_write(&dir.path().join("src/sub/deep/x.rs")), Verdict::Allow);
}

#[test]
fn write_outside_blast_radius_denied() {
    let dir = tempfile::tempdir().unwrap();
    let intc = Interceptor::for_node(&node_from(BR_NODE), None, dir.path().to_path_buf());
    match intc.check_write(&dir.path().join("src/evil.rs")) {
        Verdict::Deny(why) => assert!(why.contains("blast radius"), "why: {why}"),
        Verdict::Allow => panic!("should be denied"),
    }
}

#[test]
fn empty_blast_radius_allows_anything() {
    let dir = tempfile::tempdir().unwrap();
    let intc = Interceptor::for_node(&node_from(OPEN_NODE), None, dir.path().to_path_buf());
    assert_eq!(intc.check_write(&dir.path().join("anything.txt")), Verdict::Allow);
}

#[test]
fn command_allowlist_exact_and_wildcard() {
    let dir = tempfile::tempdir().unwrap();
    let intc = Interceptor::for_node(&node_from(BR_NODE), None, dir.path().to_path_buf());
    assert_eq!(intc.check_command("cargo test"), Verdict::Allow);
    assert_eq!(intc.check_command("cargo build --release"), Verdict::Allow);
    match intc.check_command("rm -rf /") {
        Verdict::Deny(why) => assert!(why.contains("cmd_allowlist"), "why: {why}"),
        Verdict::Allow => panic!("rm must be denied"),
    }
}

#[test]
fn no_allowlist_denies_all_commands() {
    let dir = tempfile::tempdir().unwrap();
    let intc = Interceptor::for_node(&node_from(OPEN_NODE), None, dir.path().to_path_buf());
    match intc.check_command("cargo test") {
        Verdict::Deny(why) => assert!(why.contains("cmd_allowlist"), "why: {why}"),
        Verdict::Allow => panic!("no allowlist → deny"),
    }
}

#[test]
fn network_blocked_reflects_node_flag() {
    let dir = tempfile::tempdir().unwrap();
    let blocked = Interceptor::for_node(&node_from(BR_NODE), None, dir.path().to_path_buf());
    assert!(blocked.network_blocked(), "default network=false → blocked");
    let open = Interceptor::for_node(&node_from(OPEN_NODE), None, dir.path().to_path_buf());
    assert!(!open.network_blocked(), "network=true → not blocked");
}
