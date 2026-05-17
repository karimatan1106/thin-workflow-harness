//! `harness query <subcommand>` ファサードの結線確認 ── outline ベースのみ（rust-analyzer 不要）。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(args)
        .current_dir(manifest_dir())
        .output()
        .expect("spawn harness")
}

fn out_str(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn query_help_lists_seven_subcommands() {
    let o = run(&["query", "--help"]);
    assert!(o.status.success(), "query --help failed: {}", out_str(&o));
    let s = out_str(&o);
    for sub in ["outline", "symbol", "refs", "callers", "closure", "impacted-by", "tested-by"] {
        assert!(s.contains(sub), "query --help missing {sub}: {s}");
    }
}

#[test]
fn top_help_still_lists_legacy_aliases() {
    let o = run(&["--help"]);
    let s = out_str(&o);
    // legacy alias は完全維持
    for c in ["outline", "find-symbol", "refs", "callers", "closure", "impacted-by", "tested-by", "query"] {
        assert!(s.contains(c), "top --help missing {c}: {s}");
    }
}

#[test]
fn query_outline_matches_legacy_outline() {
    let fixture = Path::new("tests/fixtures/sample_workspace_rust/src/lib.rs")
        .to_str()
        .unwrap()
        .to_string();
    let a = run(&["outline", &fixture]);
    let b = run(&["query", "outline", &fixture]);
    assert!(a.status.success(), "outline failed: {}", out_str(&a));
    assert!(b.status.success(), "query outline failed: {}", out_str(&b));
    assert_eq!(
        String::from_utf8_lossy(&a.stdout),
        String::from_utf8_lossy(&b.stdout),
        "alias と query 配下で stdout が一致しない"
    );
}

#[test]
fn query_symbol_help_advertises_daemon_flags() {
    let o = run(&["query", "symbol", "--help"]);
    assert!(o.status.success(), "query symbol --help failed: {}", out_str(&o));
    let s = out_str(&o);
    assert!(s.contains("--daemon-port"), "query symbol --help missing --daemon-port: {s}");
    assert!(s.contains("--use-daemon"), "query symbol --help missing --use-daemon: {s}");
}

#[test]
fn query_refs_callers_closure_impacted_tested_advertise_daemon_flags() {
    for sub in ["refs", "callers", "closure", "impacted-by", "tested-by"] {
        let o = run(&["query", sub, "--help"]);
        assert!(o.status.success(), "query {sub} --help failed: {}", out_str(&o));
        let s = out_str(&o);
        assert!(s.contains("--daemon-port"), "query {sub} --help missing --daemon-port: {s}");
        assert!(s.contains("--use-daemon"), "query {sub} --help missing --use-daemon: {s}");
    }
}
