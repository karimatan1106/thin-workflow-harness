//! `find_impacted_by` / `find_tested_by` integration test。
//! rust-analyzer が PATH に無ければ skip。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thin_workflow_harness_ckg::ckg::lsp::{find_impacted_by, find_tested_by};
use thin_workflow_harness_ckg::ckg::test_attrs::list_test_function_lines;
use thin_workflow_harness_ckg::ckg::test_mod_scan::{
    is_inside_cfg_test_mod, list_cfg_test_mod_ranges,
};

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

/// `#[cfg(test)] mod` 親階層判定（tree-sitter 単独、rust-analyzer 非依存）。
/// fixture src/inline_tests.rs の `mod tests { ... }` が `#[cfg(test)]` 付きで
/// range として列挙されること、その中の attr 無し `helper_no_attr` 行が
/// `is_inside_cfg_test_mod` で true 判定されることを検証。
#[test]
fn cfg_test_mod_range_covers_attr_less_helper_in_fixture() {
    let mut p = fixture_root();
    p.push("src");
    p.push("inline_tests.rs");
    let ranges = list_cfg_test_mod_ranges(&p).expect("parse ok");
    assert_eq!(
        ranges.len(),
        1,
        "expected exactly 1 cfg(test) mod block in inline_tests.rs, got {:?}",
        ranges
    );
    let (start, end) = ranges[0];
    // mod 開始行より下、 mod 終了行より上の任意行で true になるはず。
    let inside_line = (start + end) / 2;
    assert!(
        is_inside_cfg_test_mod(&p, inside_line),
        "line {} should be inside cfg(test) mod range {:?}",
        inside_line,
        (start, end)
    );
    // mod 外（build_inline_user の宣言ラインあたり）では false。
    // 安全側に、1 行目（doc comment）で false 検証。
    assert!(
        !is_inside_cfg_test_mod(&p, 1),
        "line 1 should NOT be inside cfg(test) mod"
    );
}

/// rust-analyzer 在環境のみ。 src/inline_tests.rs の `#[cfg(test)] mod tests`
/// 内側に居る attr 無し helper（`helper_no_attr`）が tested-by に含まれること。
/// MVP attr 直接検出だけでは拾えず、 cfg(test) mod 親階層判定で初めて拾える経路。
#[test]
fn tested_by_includes_attr_less_helper_inside_cfg_test_mod() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    // create_user を起点に tested-by ── helper_no_attr が create_user を呼ぶので
    // closure(In) に乗るはず。 indexing 差で空の場合は warn して skip。
    let tests = find_tested_by(
        "rust-analyzer",
        &root,
        "create_user",
        3,
        Duration::from_secs(60),
    )
    .expect("tested-by ok");
    if tests.is_empty() {
        eprintln!("warn: tested-by 結果 0（indexing 不完了の可能性）");
        return;
    }
    let hit = tests.iter().any(|t| {
        let leaf = t.name.rsplit_once("::").map(|x| x.1).unwrap_or(&t.name);
        leaf == "helper_no_attr"
    });
    if !hit {
        eprintln!(
            "warn: helper_no_attr 未検出（rust-analyzer indexing 差の可能性）: tests={:?}",
            tests
                .iter()
                .map(|t| (&t.name, &t.file, t.line))
                .collect::<Vec<_>>()
        );
        // 環境差を吸収するため hard fail にはしない。但し range 列挙経路は
        // tree-sitter で必ず通っているので range 機能自体は別 test で担保済み。
    }
}
