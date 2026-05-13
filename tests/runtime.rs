//! runtime ループ（スクリプト worker 駆動）の結合テスト ── `harness` バイナリを subprocess で叩く。

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
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn setup() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir(&fixtures(), tmp.path());
    let o = run(tmp.path(), &["start", "runtime test"]);
    assert!(o.status.success(), "start failed: {}", out_str(&o));
    tmp
}

#[test]
fn happy_path_walks_to_done() {
    let tmp = setup();
    let o = run(tmp.path(), &["run", "--script", "script_happy.toml"]);
    let s = out_str(&o);
    assert!(o.status.success(), "run should succeed: {s}");
    assert!(s.contains("status: done"), "expected done: {s}");
    // 両ノードを spawn したログ。
    assert!(s.contains("[node node1]"), "{s}");
    assert!(s.contains("[node node2]"), "{s}");
    // 最終 status も done。
    let o = run(tmp.path(), &["status"]);
    assert!(out_str(&o).contains("完了"), "{}", out_str(&o));
}

#[test]
fn respawn_after_reject_then_advances() {
    let tmp = setup();
    let o = run(tmp.path(), &["run", "--script", "script_respawn.toml"]);
    let s = out_str(&o);
    assert!(o.status.success(), "run should eventually succeed: {s}");
    assert!(s.contains("advance_rejected"), "expected a reject: {s}");
    assert!(s.contains("再 spawn"), "expected respawn log: {s}");
    // 再 spawn 時の context バンドルに直前の failed_gates（evidence_recorded）が入る。
    assert!(s.contains("直前 failed_gates: evidence_recorded"), "expected failed_gates in bundle: {s}");
    assert!(s.contains("status: done"), "{s}");
}

#[test]
fn fresh_context_per_node_spec_slice() {
    // node1 は serves=["F-001"]、node2 は serves=[] ── node2 の spec スライスに F-001 は現れない。
    let tmp = setup();
    let o = run(tmp.path(), &["run", "--script", "script_happy.toml"]);
    let s = out_str(&o);
    assert!(o.status.success(), "{s}");
    // node1 のバンドル直後の "spec:" 行に F-001。
    let n1_block = s.split("[node node1] context バンドル").nth(1).unwrap_or("");
    assert!(n1_block.lines().take(4).any(|l| l.contains("F-001")), "node1 spec slice missing F-001: {s}");
    // node2 のバンドル直後の "spec:" 行に F-001 は無い（serve しない旨）。
    let n2_block = s.split("[node node2] context バンドル").nth(1).unwrap_or("");
    let n2_spec_line = n2_block.lines().find(|l| l.trim_start().starts_with("spec:")).unwrap_or("");
    assert!(!n2_spec_line.contains("F-001"), "node2 spec slice should not mention F-001: {n2_spec_line}");
}

#[test]
fn on_reject_escalates_to_human() {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir(&fixtures(), tmp.path());
    // on_reject 付き workflow に差し替え。
    std::fs::copy(
        fixtures().join("workflow_onreject.toml"),
        tmp.path().join("workflow.toml"),
    )
    .unwrap();
    let o = run(tmp.path(), &["start", "onreject test"]);
    assert!(o.status.success(), "start: {}", out_str(&o));

    let o = run(tmp.path(), &["run", "--script", "script_onreject.toml"]);
    let s = out_str(&o);
    assert!(!o.status.success(), "should exit 1 on human escalation: {s}");
    assert!(s.contains("人間の判断待ち") || s.contains("escalation"), "{s}");
    // escalation 質問がキューに積まれている。
    let o = run(tmp.path(), &["questions"]);
    assert!(out_str(&o).contains("escalation") || out_str(&o).contains("エスカレ"), "{}", out_str(&o));
}

#[test]
fn stuck_action_blocks_on_human() {
    let tmp = setup();
    let o = run(tmp.path(), &["run", "--script", "script_stuck.toml"]);
    let s = out_str(&o);
    assert!(!o.status.success(), "stuck should exit 1: {s}");
    assert!(s.contains("詰まった") || s.contains("人間の判断待ち"), "{s}");
    let o = run(tmp.path(), &["questions"]);
    let qs = out_str(&o);
    assert!(qs.contains("stuck") || qs.contains("escalation"), "{qs}");
}

#[test]
fn abandoned_run_cannot_be_driven() {
    let tmp = setup();
    // run id を questions から拾うのは面倒なので status から run_id を取る。
    let st = out_str(&run(tmp.path(), &["status"]));
    let run_id = st
        .lines()
        .find_map(|l| l.strip_prefix("run_id : "))
        .expect("run_id line")
        .trim()
        .to_string();
    let o = run(tmp.path(), &["abandon", &run_id, "--reason", "test abandon"]);
    assert!(o.status.success(), "abandon: {}", out_str(&o));
    let o = run(tmp.path(), &["run", "--script", "script_happy.toml"]);
    let s = out_str(&o);
    assert!(!o.status.success(), "driving abandoned run should fail: {s}");
    assert!(s.contains("放棄"), "{s}");
}

#[test]
fn missing_step_for_node_is_stuck() {
    // 空スクリプト（node1 に対応 step が無い）→ stuck。
    let tmp = setup();
    let script = tmp.path().join("empty_script.toml");
    std::fs::write(&script, "# no steps\n").unwrap();
    let o = run(tmp.path(), &["run", "--script", "empty_script.toml"]);
    let s = out_str(&o);
    assert!(!o.status.success(), "should fail: {s}");
    assert!(s.contains("詰まった") || s.contains("人間の判断待ち"), "{s}");
}

#[test]
fn help_lists_run_command() {
    let tmp = tempfile::tempdir().unwrap();
    let o = run(tmp.path(), &["--help"]);
    assert!(out_str(&o).contains("run"), "help missing run: {}", out_str(&o));
}
