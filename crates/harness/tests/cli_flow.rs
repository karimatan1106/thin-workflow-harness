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

/// 選択肢付き質問の回答バリデーション（A）と一覧表示（B）の結合テスト。
///
/// options を定義した質問に対し:
/// - 範囲外の回答は失敗し、有効な選択肢を提示する
/// - 有効な選択肢（本文・index どちらでも）は成功する
/// - questions 一覧に選択肢ラベルが表示される
#[test]
fn ask_with_options_validates_answer() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    copy_dir(&fixtures(), home);

    let o = run(home, &["start", "ask options test"]);
    assert!(o.status.success(), "start failed: {}", out_str(&o));

    // 選択肢付き必須質問を積む
    let o = run(
        home,
        &["ask", "Pick one", "--option", "apple", "--option", "banana", "--required"],
    );
    assert!(o.status.success(), "ask failed: {}", out_str(&o));

    // questions 一覧に選択肢ラベルが見えること（表示改善 B）
    let o = run(home, &["questions"]);
    let s = out_str(&o);
    assert!(s.contains("Pick one"), "questions missing prompt: {s}");
    assert!(s.contains("選択肢"), "questions missing options label: {s}");
    assert!(s.contains("apple") && s.contains("banana"), "options missing: {s}");

    // 範囲外の回答は弾かれ、有効な選択肢を提示する（バリデーション A）
    let o = run(home, &["answer", "q1", "cherry"]);
    assert!(!o.status.success(), "invalid answer should fail: {}", out_str(&o));
    let e = out_str(&o);
    assert!(e.contains("無効"), "expected rejection message: {e}");
    assert!(e.contains("apple") && e.contains("banana"), "expected valid options in error: {e}");

    // 弾かれた後も未回答のまま残る
    let o = run(home, &["questions"]);
    assert!(out_str(&o).contains("Pick one"), "question should still be pending");

    // 選択肢 index で有効回答 → 本文に展開され成功する
    let o = run(home, &["answer", "q1", "1"]);
    assert!(o.status.success(), "valid index answer failed: {}", out_str(&o));
    assert!(out_str(&o).contains("banana"), "index should expand to body: {}", out_str(&o));
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

// ── F-1: 4 新コマンドの受入基準(AC-1/AC-2/AC-3)。実装前は fail する。 ──

/// AC-1: `harness spec F-001` が requirement の text と紐づく AC を表示する。
#[test]
fn spec_shows_requirement_and_acceptance() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    copy_dir(&fixtures(), home);
    // spec viewer は run 不要(spec.toml を読むだけ)
    let o = run(home, &["spec", "F-001"]);
    assert!(o.status.success(), "spec F-001 failed: {}", out_str(&o));
    let s = out_str(&o);
    assert!(s.contains("F-001"), "spec missing id: {s}");
    assert!(s.contains("fixture requirement"), "spec missing text: {s}");
    assert!(s.contains("AC-1"), "spec missing linked acceptance: {s}");
}

/// AC-1: 存在しない requirement は非ゼロで終了する。
#[test]
fn spec_unknown_requirement_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    copy_dir(&fixtures(), home);
    let o = run(home, &["spec", "F-999"]);
    assert!(!o.status.success(), "unknown requirement should fail: {}", out_str(&o));
}

/// AC-2: `harness artifact <name>` が path を表示、`artifact-list` が一覧、未登録は非ゼロ。
#[test]
fn artifact_view_and_list() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    copy_dir(&fixtures(), home);
    let o = run(home, &["start", "artifact test"]);
    assert!(o.status.success(), "start failed: {}", out_str(&o));
    std::fs::write(home.join("note.md"), "hi").unwrap();
    let o = run(home, &["record-artifact", "mynote", "note.md"]);
    assert!(o.status.success(), "record-artifact failed: {}", out_str(&o));

    let o = run(home, &["artifact", "mynote"]);
    assert!(o.status.success(), "artifact failed: {}", out_str(&o));
    assert!(out_str(&o).contains("note.md"), "artifact missing path: {}", out_str(&o));

    let o = run(home, &["artifact-list"]);
    assert!(o.status.success(), "artifact-list failed: {}", out_str(&o));
    assert!(out_str(&o).contains("mynote"), "artifact-list missing name: {}", out_str(&o));

    let o = run(home, &["artifact", "ghost"]);
    assert!(!o.status.success(), "unknown artifact should fail: {}", out_str(&o));
}

/// AC-3: `harness stuck "<理由>"` が記録され status に反映される。
#[test]
fn stuck_records_and_shows_in_status() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    copy_dir(&fixtures(), home);
    let o = run(home, &["start", "stuck test"]);
    assert!(o.status.success(), "start failed: {}", out_str(&o));
    let o = run(home, &["stuck", "blocked on X"]);
    assert!(o.status.success(), "stuck failed: {}", out_str(&o));
    let o = run(home, &["status"]);
    let s = out_str(&o);
    assert!(
        s.contains("blocked on X") || s.to_lowercase().contains("stuck"),
        "status missing stuck: {s}"
    );
}
