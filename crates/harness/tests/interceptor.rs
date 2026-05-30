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

// ── path traversal 防御（safe_resolve）の回帰テスト ──
// lexical_normalize は実 fs に触れないが、cwd は実在 dir を渡しておく。

#[test]
fn safe_resolve_allows_relative_under_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let intc = Interceptor::for_node(&node_from(OPEN_NODE), None, dir.path().to_path_buf());
    let r = intc.safe_resolve("src/main.rs").expect("cwd 配下は許可されるべき");
    assert!(r.starts_with(dir.path()), "解決結果が cwd 配下でない: {r:?}");
}

#[test]
fn safe_resolve_rejects_parent_escape() {
    let dir = tempfile::tempdir().unwrap();
    let intc = Interceptor::for_node(&node_from(OPEN_NODE), None, dir.path().to_path_buf());
    // `..` 連打で cwd の外（認証情報など）を読もうとする典型的 traversal。
    let r = intc.safe_resolve("../../../Users/owner/.claude/.credentials.json");
    assert!(r.is_err(), "親ディレクトリ脱出は拒否されるべきだった: {r:?}");
}

#[test]
fn safe_resolve_rejects_absolute_outside_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let intc = Interceptor::for_node(&node_from(OPEN_NODE), None, dir.path().to_path_buf());
    let abs = if cfg!(windows) { "C:/Windows/System32/config" } else { "/etc/passwd" };
    let r = intc.safe_resolve(abs);
    assert!(r.is_err(), "cwd 外の絶対パスは拒否されるべきだった: {r:?}");
}

#[test]
fn safe_resolve_normalizes_interior_dotdot() {
    let dir = tempfile::tempdir().unwrap();
    let intc = Interceptor::for_node(&node_from(OPEN_NODE), None, dir.path().to_path_buf());
    // 途中に `..` があっても最終的に cwd 配下なら許可（src/sub/../main.rs == src/main.rs）。
    let r = intc.safe_resolve("src/sub/../main.rs").expect("内部 .. 正規化後 cwd 配下なら許可");
    assert!(r.ends_with("src/main.rs"), "内部 .. の正規化が誤り: {r:?}");
    assert!(r.starts_with(dir.path()));
}
