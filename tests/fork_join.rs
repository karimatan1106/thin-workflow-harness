//! fork/join 並列実行の結合テスト（scripted worker 経路）。
//!
//! 既存 `runtime.rs` と同じく `harness` バイナリを subprocess で叩く。fork ノードを
//! entry に持つ workflow に差し替えて `start` → `run --script <fork>` を回し、blast
//! radius 違反 / artifact 合流 / 1 branch fail の3経路を確認する。
//!
//! 注: case4/case5（fork→jn dispatch & branch metrics 回帰テスト）は 200 行制限の
//! ため `tests/fork_join_dispatch.rs` に分離してある。

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

/// fork 用 home を組み立てる ── fixtures をコピーした上で workflow.toml と spec.toml
/// を fork 向けに差し替え、`start` まで済ませる。
fn setup_fork(workflow_fixture: &str) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir(&fixtures(), tmp.path());
    std::fs::copy(
        fixtures().join(workflow_fixture),
        tmp.path().join("workflow.toml"),
    )
    .unwrap();
    std::fs::copy(
        fixtures().join("spec_fork.toml"),
        tmp.path().join("spec.toml"),
    )
    .unwrap();
    let o = run(tmp.path(), &["start", "fork test"]);
    assert!(o.status.success(), "start failed: {}", out_str(&o));
    tmp
}

/// 直近 main run の jsonl を読んで、指定 type の行があるかと payload を簡易に拾う。
fn read_main_log(home: &Path) -> String {
    let state = home.join("state");
    let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in std::fs::read_dir(&state).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("jsonl") {
            continue;
        }
        // サイドカー（stem に `.`）は除外。
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if stem.contains('.') {
            continue;
        }
        let mtime = e.metadata().unwrap().modified().unwrap();
        if latest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
            latest = Some((mtime, p));
        }
    }
    let path = latest.expect("no main run jsonl found").1;
    std::fs::read_to_string(&path).unwrap()
}

#[test]
fn case1_fork_overlap_blast_radius_rejected() {
    // b1 / b2 が同じ F-001 を serves するので blast_radius_disjoint 違反。
    // run はノード fork1 を進められずエラー終了する。
    let tmp = setup_fork("workflow_fork_overlap.toml");
    let o = run(tmp.path(), &["run", "--script", "script_fork_happy.toml"]);
    let s = out_str(&o);
    assert!(!o.status.success(), "fork with overlap should fail: {s}");
    assert!(
        s.contains("blast_radius_disjoint"),
        "expected blast_radius_disjoint mention: {s}"
    );
    // overlap した F-001 が原因として現れる。
    assert!(s.contains("F-001") || s.contains("共有"), "expected overlap detail: {s}");
    // BranchForked / BranchJoined は書かれない（事前チェックで弾かれるため）。
    let log = read_main_log(tmp.path());
    assert!(
        !log.contains("\"branch_forked\""),
        "branch_forked should not be written when blast radius rejected: {log}"
    );
}

#[test]
fn case2_fork_disjoint_artifacts_merged() {
    // b1 が out1、b2 が out2 を register。BranchJoined(success) 時に main state へ fold-in
    // されるので、`status` 出力に両方の artifact が出てくる。
    let tmp = setup_fork("workflow_fork_disjoint.toml");
    let o = run(tmp.path(), &["run", "--script", "script_fork_happy.toml"]);
    let s = out_str(&o);
    assert!(o.status.success(), "disjoint fork should succeed: {s}");
    assert!(s.contains("[fork fork1]"), "expected fork log: {s}");
    assert!(s.contains("[branch b1] done"), "expected b1 done: {s}");
    assert!(s.contains("[branch b2] done"), "expected b2 done: {s}");
    assert!(s.contains("status: done"), "expected runtime done: {s}");

    // status コマンドで artifact が両方見える。
    let st = run(tmp.path(), &["status"]);
    let stxt = out_str(&st);
    assert!(stxt.contains("out1"), "status missing out1: {stxt}");
    assert!(stxt.contains("out2"), "status missing out2: {stxt}");

    // メインログに branch_joined success が記録される。
    let log = read_main_log(tmp.path());
    assert!(log.contains("\"branch_forked\""), "missing branch_forked event: {log}");
    assert!(
        log.contains("\"branch_joined\"") && log.contains("\"status\":\"success\""),
        "missing branch_joined success: {log}"
    );
}

#[test]
fn case3_fork_one_branch_fail_records_failure() {
    // b1 success, b2 stuck → BranchJoined(failed, failures=[..]) が書かれて run はエラー終了。
    let tmp = setup_fork("workflow_fork_disjoint.toml");
    let o = run(tmp.path(), &["run", "--script", "script_fork_one_fails.toml"]);
    let s = out_str(&o);
    assert!(!o.status.success(), "1-branch-fail should fail run: {s}");
    assert!(s.contains("branch(es) failed"), "expected aggregated failure message: {s}");
    assert!(s.contains("b2"), "expected b2 in failure message: {s}");

    let log = read_main_log(tmp.path());
    assert!(log.contains("\"branch_forked\""), "missing branch_forked: {log}");
    assert!(
        log.contains("\"branch_joined\"") && log.contains("\"status\":\"failed\""),
        "missing branch_joined failed: {log}"
    );
}
