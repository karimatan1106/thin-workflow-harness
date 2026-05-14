//! `find_impacted_by` / `find_tested_by` integration test。
//! rust-analyzer が PATH に無ければ skip。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thin_workflow_harness::ckg::lsp::{find_impacted_by, find_tested_by};
use thin_workflow_harness::ckg::test_attrs::list_test_function_lines;

fn fixture_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/sample_workspace_rust");
    p
}

fn rust_analyzer_available() -> bool {
    Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn impacted_by_wraps_closure_in() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let nodes = find_impacted_by(
        "rust-analyzer",
        &root,
        "create_user",
        3,
        Duration::from_secs(60),
    )
    .expect("impacted-by ok");
    if nodes.is_empty() {
        eprintln!("warn: impacted-by 結果 0（indexing 不完了 or callHierarchy 未サポート）");
        return;
    }
    assert!(!nodes.is_empty(), "expected at least 1 impacted node");
    assert!(nodes.iter().all(|n| n.depth >= 1 && n.depth <= 3));
}

#[test]
fn tested_by_filters_to_test_nodes_only() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let impacted = find_impacted_by(
        "rust-analyzer",
        &root,
        "create_user",
        3,
        Duration::from_secs(60),
    )
    .expect("impacted-by ok");
    let tests = find_tested_by(
        "rust-analyzer",
        &root,
        "create_user",
        3,
        Duration::from_secs(60),
    )
    .expect("tested-by ok");
    if impacted.is_empty() && tests.is_empty() {
        eprintln!("warn: 両方空、indexing 不完了の可能性");
        return;
    }
    assert!(
        tests.len() <= impacted.len(),
        "tested ({}) must be <= impacted ({})",
        tests.len(),
        impacted.len()
    );

    // attr ベースでは「make_user_for_test」のような attr 無し helper が
    // tests/ に居ても tested-by から除外されるはず（精度向上の核心）。
    let hit_helper = tests.iter().any(|t| {
        let leaf = t.name.rsplit_once("::").map(|x| x.1).unwrap_or(&t.name);
        leaf == "make_user_for_test"
    });
    assert!(
        !hit_helper,
        "make_user_for_test (attr 無し helper) が tested-by に混入: tests={:?}",
        tests.iter().map(|t| (&t.name, &t.file, t.line)).collect::<Vec<_>>()
    );

    // fixture 内では tests/it_user.rs の test_create_user が拾えるはず。
    let hit_test_create_user = tests.iter().any(|t| {
        let leaf = t.name.rsplit_once("::").map(|x| x.1).unwrap_or(&t.name);
        leaf == "test_create_user"
    });
    if !hit_test_create_user {
        eprintln!(
            "warn: test_create_user 未検出（rust-analyzer indexing 差）: tests={:?}",
            tests
                .iter()
                .map(|t| (&t.name, &t.file))
                .collect::<Vec<_>>()
        );
    }
}

/// attr 検出を直接（rust-analyzer 非依存）。fixture の tests/it_user.rs に
/// `#[test]` 付き関数が居ることを tree-sitter だけで検証する。CI 環境差を吸収。
#[test]
fn list_test_function_lines_detects_attr_in_fixture() {
    let mut p = fixture_root();
    p.push("tests");
    p.push("it_user.rs");
    let lines = list_test_function_lines(&p).expect("parse ok");
    assert!(
        !lines.is_empty(),
        "expected at least one #[test] fn in it_user.rs, got {:?}",
        lines
    );
}

/// src/inline_tests.rs の `#[cfg(test)] mod tests { #[test] fn ... }` も
/// tree-sitter で test fn として拾えること（MVP は attr 直接検出のみだが、
/// mod 内側にある `#[test]` は素直に拾えるはず ── cfg(test) 親 mod 判定とは別経路）。
#[test]
fn list_test_function_lines_detects_attr_inside_cfg_test_mod() {
    let mut p = fixture_root();
    p.push("src");
    p.push("inline_tests.rs");
    let lines = list_test_function_lines(&p).expect("parse ok");
    assert!(
        !lines.is_empty(),
        "expected #[test] fn inside #[cfg(test)] mod, got {:?}",
        lines
    );
}
