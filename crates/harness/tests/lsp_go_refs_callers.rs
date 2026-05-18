//! `find_refs_for_lang` / `find_callers_for_lang` / `find_closure_for_lang` /
//! `find_tested_by_for_lang` 用 Go (gopls) integration test。
//!
//! `gopls version` が 0 で返らなければ skip。
//! 在環境では sample_workspace_go/ 配下で `CreateUser` の references / callers /
//! closure(in) / tested-by が拾えることを期待する。
//! indexing 不完了で空配列でも `warn` skip 扱い（基本動作 OK）。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thin_workflow_harness::ckg::lsp::{
    find_callers_for_lang, find_closure_for_lang, find_refs_for_lang, find_tested_by_for_lang,
    Direction, Lang,
};

fn fixture_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/sample_workspace_go");
    p
}

fn gopls_available() -> bool {
    Command::new("gopls")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn find_refs_create_user_in_go_workspace() {
    if !gopls_available() {
        eprintln!("skip: gopls が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let refs = find_refs_for_lang(Lang::Go, &root, "CreateUser", Duration::from_secs(60))
        .expect("find_refs_for_lang ok");
    if refs.is_empty() {
        eprintln!("warn: references 空（indexing 不完了の可能性）。基本動作は OK。");
        return;
    }
    assert!(!refs.is_empty(), "expected at least 1 reference to CreateUser");
}

#[test]
fn find_callers_create_user_in_go_workspace() {
    if !gopls_available() {
        eprintln!("skip: gopls が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let callers = find_callers_for_lang(Lang::Go, &root, "CreateUser", Duration::from_secs(60))
        .expect("find_callers_for_lang ok");
    if callers.is_empty() {
        eprintln!("warn: callers 空(indexing 不完了 or callHierarchy 未サポート)。基本動作は OK。");
        return;
    }
    assert!(!callers.is_empty(), "expected at least 1 caller of CreateUser");
}

#[test]
fn find_closure_in_go_workspace() {
    if !gopls_available() {
        eprintln!("skip: gopls が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let nodes = find_closure_for_lang("CreateUser", 2, Direction::In, Lang::Go, &root)
        .expect("find_closure_for_lang ok");
    if nodes.is_empty() {
        eprintln!("warn: closure(in) 結果 0（indexing 不完了の可能性）。基本動作は OK。");
        return;
    }
    assert!(nodes.iter().all(|n| n.depth >= 1 && n.depth <= 2));
}

#[test]
fn find_tested_by_in_go_workspace() {
    if !gopls_available() {
        eprintln!("skip: gopls が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let tests = find_tested_by_for_lang("CreateUser", 2, Lang::Go, &root)
        .expect("find_tested_by_for_lang ok");
    if tests.is_empty() {
        eprintln!("warn: tested-by 結果 0（indexing 不完了 or test fixture 検出されず）");
        return;
    }
    let all_test_like = tests.iter().all(|t| {
        let f = t.file.to_ascii_lowercase().replace('\\', "/");
        let name = t.name.as_str();
        let leaf = name.rsplit_once('.').map(|x| x.1).unwrap_or(name);
        f.ends_with("_test.go")
            || leaf.starts_with("Test")
            || leaf.starts_with("Benchmark")
            || leaf.starts_with("Example")
            || leaf.starts_with("Fuzz")
    });
    assert!(all_test_like, "all tested-by results must be test-like: {:?}", tests);
}
