//! CLI フローの結合テスト ── `harness` バイナリを subprocess で叩く（env が分離される）。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), &to).unwrap();
        }
    }
}

fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(args)
        .env("HARNESS_HOME", home)
        .env_remove("HARNESS_RUN")
        .current_dir(home)
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
fn full_cli_flow() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    copy_dir(&fixtures(), home);

    // validate
    let o = run(home, &["validate"]);
    assert!(o.status.success(), "validate failed: {}", out_str(&o));
    assert!(out_str(&o).contains("OK"));

    // start
    let o = run(home, &["start", "smoke test"]);
    assert!(o.status.success(), "start failed: {}", out_str(&o));
    let s = out_str(&o);
    assert!(s.contains("node1"), "status missing node1: {s}");
    assert!(s.contains("n1.md"), "status missing skill path: {s}");

    // status shows node1, gate FAIL (evidence_recorded done1)
    let o = run(home, &["status"]);
    let s = out_str(&o);
    assert!(s.contains("[FAIL] evidence_recorded"), "{s}");

    // advance -> rejected, exit non-zero
    let o = run(home, &["advance"]);
    assert!(!o.status.success(), "advance should be rejected: {}", out_str(&o));
    assert!(out_str(&o).contains("却下"));

    // report-evidence done1
    let o = run(home, &["report-evidence", "done1", "{}"]);
    assert!(o.status.success(), "report-evidence failed: {}", out_str(&o));

    // advance -> node2
    let o = run(home, &["advance"]);
    assert!(o.status.success(), "advance to node2 failed: {}", out_str(&o));
    assert!(out_str(&o).contains("node2"), "{}", out_str(&o));

    // skill shows node2 skill path
    let o = run(home, &["skill"]);
    assert!(out_str(&o).contains("n2.md"), "{}", out_str(&o));

    // reset -> back to node1
    let o = run(home, &["reset", "--yes"]);
    assert!(o.status.success(), "reset failed: {}", out_str(&o));
    assert!(out_str(&o).contains("node1"), "{}", out_str(&o));

    // reset without --yes -> error
    let o = run(home, &["reset"]);
    assert!(!o.status.success());

    // back at node1 -> error (先頭)
    let o = run(home, &["back", "nope"]);
    assert!(!o.status.success());

    // gates at node1 lists the FAIL
    let o = run(home, &["gates"]);
    assert!(out_str(&o).contains("evidence_recorded"), "{}", out_str(&o));
}

#[test]
fn status_without_run_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    copy_dir(&fixtures(), home);
    let o = run(home, &["status"]);
    assert!(!o.status.success());
    assert!(out_str(&o).contains("no runs found"), "{}", out_str(&o));
}

#[test]
fn help_lists_new_commands() {
    let tmp = tempfile::tempdir().unwrap();
    let o = run(tmp.path(), &["--help"]);
    let s = out_str(&o);
    for c in ["start", "status", "advance", "back", "record-artifact", "report-evidence", "reset", "validate", "skill", "gates"] {
        assert!(s.contains(c), "help missing {c}: {s}");
    }
}
