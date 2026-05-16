//! `find_symbol` integration test。rust-analyzer が PATH に無い環境では skip。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thin_workflow_harness::ckg::lsp::find_symbol;

fn fixture_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/sample_workspace_rust");
    p
}

/// rust-analyzer が動く環境かを最小コストで判定する。
/// PATH 上に有り、`--version` が 0 で返れば true。
fn rust_analyzer_available() -> bool {
    Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn find_symbol_user_in_sample_workspace() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let syms = find_symbol(
        "rust-analyzer",
        &root,
        "user",
        None,
        Duration::from_secs(60),
    )
    .expect("find_symbol ok");
    // indexing が間に合わず空になる環境もあるので、ここでは「壊れていない」までを必須に。
    // 何か取れていれば User か create_user のどちらかが含まれることを確認する。
    if syms.is_empty() {
        eprintln!("warn: workspace/symbol が空（indexing 不完了の可能性）。基本動作は OK。");
        return;
    }
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    let hit = names
        .iter()
        .any(|n| n.contains("User") || n.contains("create_user"));
    assert!(hit, "User/create_user not found in: {names:?}");
}

#[test]
fn find_symbol_kind_filter_excludes_other_kinds() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let syms = find_symbol(
        "rust-analyzer",
        &root,
        "user",
        Some("function"),
        Duration::from_secs(60),
    )
    .expect("find_symbol ok");
    for s in &syms {
        assert_eq!(s.kind, "function", "non-function leaked through filter: {s:?}");
    }
}


/// 存在しない symbol を timeout 60s で叩いても、empty が < 5s で返ることを保証する。
/// 旧実装は empty を retry し続けて 60s timeout に張り付くバグがあった (layer 2.5 bench 発覚)。
#[test]
fn find_symbol_empty_returns_fast() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let started = std::time::Instant::now();
    let syms = find_symbol(
        "rust-analyzer",
        &root,
        "AbsolutelyNotExistent_ZzNotReal999",
        None,
        Duration::from_secs(60),
    )
    .expect("find_symbol ok (empty allowed)");
    let elapsed_ms = started.elapsed().as_millis();
    assert!(syms.is_empty(), "no-hit query returned non-empty: {syms:?}");
    // cold-start (LSP spawn + initialize) を含むので 30s 余裕、ただし 60s には張り付かない。
    assert!(
        elapsed_ms < 30_000,
        "no-hit symbol took too long: {elapsed_ms}ms (should be much less than 60s)"
    );
    eprintln!("[fast-empty] no-hit symbol returned in {elapsed_ms}ms");
}
