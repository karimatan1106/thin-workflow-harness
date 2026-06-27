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

    // default_budget は dogfood で確定した実効下限を反映していること。
    let wf_text = std::fs::read_to_string(&wf_path).unwrap();
    assert!(
        wf_text.contains("default_budget"),
        "default_budget が workflow.toml に書かれていない: {wf_text}",
    );
    assert!(
        wf_text.contains("max_tokens = 8000"),
        "default_budget.max_tokens=8000 が無い: {wf_text}",
    );
    assert!(
        wf_text.contains("max_tool_calls = 12"),
        "default_budget.max_tool_calls=12 が無い: {wf_text}",
    );

    // 役割別モデル割り当て ── [meta].default_model と高精度ノードの Opus 上書き。
    // 版番号でなくティアエイリアス(sonnet/opus)で書く(具体 ID は resolve_model で解決)。
    assert!(
        wf_text.contains("default_model = \"sonnet\""),
        "default_model が workflow.toml に無い: {wf_text}",
    );
    assert!(
        wf_text.contains("model = \"opus\""),
        "Opus のノード上書きが無い: {wf_text}",
    );
}

#[test]
fn init_security_template_creates_single_node_workflow() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    copy_dir(&fixtures().join("repo_rust"), &repo);

    let dir_str = repo.to_string_lossy().to_string();
    let o = run(tmp.path(), &["init", &dir_str, "--force", "--template", "security"]);
    assert!(o.status.success(), "security init failed: {}", out_str(&o));

    let harness = repo.join(".harness");
    let wf_path = harness.join("workflow.toml");
    assert!(wf_path.exists(), "workflow.toml missing");
    // security skill が standalone 名で置かれること。
    assert!(harness.join("skills/security.md").exists(), "security.md missing");

    let wf_text = std::fs::read_to_string(&wf_path).unwrap();
    assert!(wf_text.contains("security-only"), "not the security template: {wf_text}");
    assert!(wf_text.contains("security_review"), "evidence gate missing: {wf_text}");
    // security テンプレは最高精度の Opus を既定に置く(ティアエイリアス)。
    assert!(
        wf_text.contains("default_model = \"opus\""),
        "security default_model が無い: {wf_text}",
    );

    // validate を通ること。
    let validate_arg = format!("--workflow={}", wf_path.display());
    let o = run(tmp.path(), &["validate", &validate_arg]);
    assert!(o.status.success(), "validate failed: {}", out_str(&o));
}

#[test]
fn init_preservation_template_creates_track() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    copy_dir(&fixtures().join("repo_rust"), &repo);

    let dir_str = repo.to_string_lossy().to_string();
    let o = run(tmp.path(), &["init", &dir_str, "--force", "--template", "preservation"]);
    assert!(o.status.success(), "preservation init failed: {}", out_str(&o));

    let harness = repo.join(".harness");
    let wf_path = harness.join("workflow.toml");
    assert!(wf_path.exists(), "workflow.toml missing");
    let wf_text = std::fs::read_to_string(&wf_path).unwrap();
    assert!(wf_text.contains("name = \"preservation\""), "not the preservation template: {wf_text}");
    assert!(wf_text.contains("differential_gate.mjs"), "differential gate missing: {wf_text}");
    assert!(wf_text.contains("reconcile_gate.mjs"), "reconcile gate missing: {wf_text}");

    // 5 skill + 6 tool + テンプレ + golden + state seed が生成されること。
    for s in ["p01-research.md", "p02-capture.md", "p03-differential.md", "p04-reconcile.md", "p05-coverage.md"] {
        assert!(harness.join("skills").join(s).exists(), "skill {s} missing");
    }
    for t in ["preservation_lib.mjs", "capture_oracle.mjs", "differential_gate.mjs", "reconcile_gate.mjs", "coverage_gate.mjs", "db-assert.mjs"] {
        assert!(harness.join("bin").join(t).exists(), "tool {t} missing");
    }
    assert!(harness.join("equivalence.json").exists(), "equivalence.json missing");
    assert!(harness.join("preservation/input_space.json").exists(), "input_space.json missing");
    assert!(harness.join("golden/manifest.json").exists(), "golden/manifest.json missing");
    assert!(harness.join("state/reconcile_ledger.json").exists(), "reconcile_ledger seed missing");

    // validate を通ること。
    let validate_arg = format!("--workflow={}", wf_path.display());
    let o = run(tmp.path(), &["validate", &validate_arg]);
    assert!(o.status.success(), "validate failed: {}", out_str(&o));
}

#[test]
fn init_unknown_template_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    copy_dir(&fixtures().join("repo_rust"), &repo);

    let dir_str = repo.to_string_lossy().to_string();
    let o = run(tmp.path(), &["init", &dir_str, "--force", "--template", "bogus"]);
    assert!(!o.status.success(), "unknown template should fail: {}", out_str(&o));
    assert!(out_str(&o).contains("未知の template"), "missing error hint: {}", out_str(&o));
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

#[test]
fn start_auto_detects_dot_harness_when_harness_home_unset() {
    // CWD/.harness/workflow.toml を harness が自動検出して start できることを確認する。
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    copy_dir(&fixtures().join("repo_rust"), &repo);

    let dir_str = repo.to_string_lossy().to_string();
    let o = run(tmp.path(), &["init", &dir_str, "--force"]);
    assert!(o.status.success(), "init failed: {}", out_str(&o));

    // HARNESS_HOME を外したまま repo 直下から harness start を叩く。
    let o = run(&repo, &["start", "auto-detect smoke"]);
    let s = out_str(&o);
    assert!(o.status.success(), "start failed: {s}");
    assert!(s.contains("run "), "missing run id line: {s}");

    // event log が .harness/state/ 配下に作られていること。
    let state = repo.join(".harness").join("state");
    let entries: Vec<_> = std::fs::read_dir(&state)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jsonl"))
        .collect();
    assert!(!entries.is_empty(), "no jsonl event log in {}", state.display());
}
