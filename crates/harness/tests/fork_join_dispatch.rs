//! fork→jn dispatch & branch metrics の回帰防止テスト（dogfood 5 で見つかった
//! 二重実行バグ・metrics 欠落の固定化）。
//!
//! 親ファイル `fork_join.rs` から 200 行制限超過で切り出した分。helpers は
//! tests/ が crate ごとに分離するため小さく複製する（共有 mod は別 crate 化が
//! 必要になり overhead に見合わない）。

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

/// 直近 main run の jsonl（サイドカー除く）を読む。
fn read_main_log(home: &Path) -> String {
    let state = home.join("state");
    let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in std::fs::read_dir(&state).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("jsonl") {
            continue;
        }
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if stem.contains('.') {
            continue;
        }
        let mtime = e.metadata().unwrap().modified().unwrap();
        if latest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
            latest = Some((mtime, p));
        }
    }
    std::fs::read_to_string(latest.expect("no main run jsonl found").1).unwrap()
}

#[test]
fn case4_fork_with_next_jumps_to_join_skipping_branches_on_main_path() {
    // fork に next=["jn"] が設定されている場合、fork 完了後 main runtime は branches を
    // 逐次再走行せず直接 jn にジャンプする（dogfood 5 二重実行バグの回帰防止）。
    let tmp = setup_fork("workflow_fork_jump.toml");
    let o = run(tmp.path(), &["run", "--script", "script_fork_jump.toml"]);
    let s = out_str(&o);
    assert!(o.status.success(), "fork jump should succeed: {s}");

    // メインログ: fork1 -> jn の Advance が書かれている（branches[0] でなく next[0]）。
    let log = read_main_log(tmp.path());
    assert!(
        log.contains("\"from\":\"fork1\"") && log.contains("\"to\":\"jn\""),
        "expected Advance from fork1 to jn (not branches[0]): {log}"
    );
    // main path で b1/b2 の Advance は出ない（fork スキップ）。
    assert!(
        !log.contains("\"from\":\"b1\""),
        "b1 must not appear on main path (fork should skip it): {log}"
    );
    assert!(
        !log.contains("\"from\":\"b2\""),
        "b2 must not appear on main path (fork should skip it): {log}"
    );
}

#[test]
fn case5_fork_branches_each_emit_metrics_entry() {
    // Scripted 経路: fork 内 branch_thread が完了時に append_metrics で 1 行追記する
    // ことを確認。dogfood 5 で fork 内 spawn の metrics 欠落が起きていた回帰防止。
    let tmp = setup_fork("workflow_fork_jump.toml");
    let o = run(tmp.path(), &["run", "--script", "script_fork_jump.toml"]);
    let s = out_str(&o);
    assert!(o.status.success(), "fork jump should succeed: {s}");

    let state = tmp.path().join("state");
    let mut metrics_path: Option<std::path::PathBuf> = None;
    for e in std::fs::read_dir(&state).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.ends_with(".metrics.jsonl") {
            metrics_path = Some(p);
        }
    }
    let mp = metrics_path.expect("metrics file missing");
    let metrics = std::fs::read_to_string(&mp).unwrap();
    let b1_lines = metrics.lines().filter(|l| l.contains("\"node\":\"b1\"")).count();
    let b2_lines = metrics.lines().filter(|l| l.contains("\"node\":\"b2\"")).count();
    assert!(b1_lines >= 1, "expected b1 metrics entry, got: {metrics}");
    assert!(b2_lines >= 1, "expected b2 metrics entry, got: {metrics}");
}
