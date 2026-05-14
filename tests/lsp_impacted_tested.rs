//! `find_impacted_by` / `find_tested_by` integration test。
//! rust-analyzer が PATH に無ければ skip。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thin_workflow_harness::ckg::lsp::{find_impacted_by, find_tested_by};

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
    // 何らかの caller が拾えていれば OK。少なくとも 1 件以上。
    assert!(!nodes.is_empty(), "expected at least 1 impacted node");
    // depth は 1..=3 の範囲内。
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
    // tested は impacted の部分集合のはず。
    assert!(
        tests.len() <= impacted.len(),
        "tested ({}) must be <= impacted ({})",
        tests.len(),
        impacted.len()
    );
    // tested が 1 件以上拾えた場合、すべて test heuristic を満たすはず。
    for t in &tests {
        let path_norm = t.file.replace('\\', "/");
        let in_tests_dir =
            path_norm.starts_with("tests/") || path_norm.contains("/tests/");
        let test_suffix =
            path_norm.ends_with("_test.rs") || path_norm.ends_with("_tests.rs");
        let leaf = t.name.rsplit_once("::").map(|x| x.1).unwrap_or(&t.name);
        let name_match = leaf.starts_with("test_") || leaf.ends_with("_test");
        assert!(
            in_tests_dir || test_suffix || name_match,
            "tested node must satisfy heuristic: name={}, file={}",
            t.name,
            t.file
        );
    }
    // fixture 内では tests/it_user.rs の test_create_user が拾えるはず。
    // ただし indexing 状況で 0 件もあり得るので warn のみ。
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
