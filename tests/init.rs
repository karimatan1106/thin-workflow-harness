//! `harness init` / `harness doctor` の最小結合テスト。
//!
//! 全シナリオは網羅しない ── walking skeleton として「Rust fixture を init して
//! workflow.toml が生成され validate を通る」「doctor が走る」「--force 無しは拒否」
//! の3点だけ確認する。残りの言語/CI 検出は次バッチに回す。

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

fn run(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(args)
        .env_remove("HARNESS_HOME")
        .env_remove("HARNESS_RUN")
        .current_dir(cwd)
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
fn init_rust_fixture_creates_harness_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    copy_dir(&fixtures().join("repo_rust"), &repo);

    let dir_str = repo.to_string_lossy().to_string();
    let o = run(tmp.path(), &["init", &dir_str, "--force"]);
    assert!(o.status.success(), "init failed: {}", out_str(&o));
    let s = out_str(&o);
    assert!(s.contains("rust"), "lang not detected: {s}");

    let harness = repo.join(".harness");
    assert!(harness.join("workflow.toml").exists(), "workflow.toml missing");
    assert!(harness.join("spec.toml").exists(), "spec.toml missing");
    assert!(harness.join(".gitignore").exists(), ".gitignore missing");
    assert!(harness.join("skills").is_dir(), "skills/ missing");
    assert!(harness.join("state/.gitkeep").exists(), "state/.gitkeep missing");

    let wf_path = harness.join("workflow.toml");
    let validate_arg = format!("--workflow={}", wf_path.display());
    let o = run(tmp.path(), &["validate", &validate_arg]);
    assert!(o.status.success(), "validate failed: {}", out_str(&o));
}

#[test]
fn init_without_force_on_existing_harness_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    copy_dir(&fixtures().join("repo_rust"), &repo);

    let dir_str = repo.to_string_lossy().to_string();
    let o = run(tmp.path(), &["init", &dir_str, "--force"]);
    assert!(o.status.success(), "first init failed: {}", out_str(&o));

    let o = run(tmp.path(), &["init", &dir_str]);
    assert!(!o.status.success(), "second init should reject: {}", out_str(&o));
    let s = out_str(&o);
    assert!(s.contains("--force"), "missing --force hint: {s}");
}

#[test]
fn doctor_runs_on_initialized_harness() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    copy_dir(&fixtures().join("repo_rust"), &repo);

    let dir_str = repo.to_string_lossy().to_string();
    let init = run(tmp.path(), &["init", &dir_str, "--force"]);
    assert!(init.status.success(), "init failed: {}", out_str(&init));

    let o = run(tmp.path(), &["doctor", &dir_str]);
    let s = out_str(&o);
    assert!(s.contains("[OK]") || s.contains("[WARN]"), "doctor produced no markers: {s}");
    assert!(s.contains("validate"), "doctor missing validate line: {s}");
}
