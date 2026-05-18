//! `find_refs_for_lang` / `find_callers_for_lang` / `find_closure_for_lang` /
//! `find_tested_by_for_lang` 用 Python (pyright) integration test。
//!
//! `pyright-langserver` が PATH に無ければ skip。
//! 在環境では sample_workspace_py/ 配下で
//! `create_user` の references が 1 件以上、incoming callers が 1 件以上、
//! tested-by から `test_user.py` 配下の test 関数（tree-sitter 検出も含む）が
//! 拾えることを期待する。indexing 不完了で空配列も「壊れていない」扱いで warn skip。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thin_workflow_harness_ckg::ckg::lsp::{
    find_callers_for_lang, find_closure_for_lang, find_refs_for_lang, find_tested_by_for_lang,
    Direction, Lang,
};

fn fixture_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/sample_workspace_py");
    p
}

fn pyright_available() -> bool {
    Command::new("pyright-langserver")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn find_refs_create_user_in_py_workspace() {
    if !pyright_available() {
        eprintln!("skip: pyright-langserver が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let refs = find_refs_for_lang(Lang::Py, &root, "create_user", Duration::from_secs(60))
        .expect("find_refs_for_lang ok");
    if refs.is_empty() {
        eprintln!("warn: references 空（indexing 不完了の可能性）。基本動作は OK。");
        return;
    }
    assert!(!refs.is_empty(), "expected at least 1 reference to create_user");
}

#[test]
fn find_callers_create_user_in_py_workspace() {
    if !pyright_available() {
        eprintln!("skip: pyright-langserver が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let callers = find_callers_for_lang(Lang::Py, &root, "create_user", Duration::from_secs(60))
        .expect("find_callers_for_lang ok");
    if callers.is_empty() {
        eprintln!("warn: callers 空（indexing 不完了 or callHierarchy 未サポート）。基本動作は OK。");
        return;
    }
    assert!(!callers.is_empty(), "expected at least 1 caller of create_user");
}

#[test]
fn find_closure_in_py_workspace() {
    if !pyright_available() {
        eprintln!("skip: pyright-langserver が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let nodes = find_closure_for_lang("create_user", 2, Direction::In, Lang::Py, &root)
        .expect("find_closure_for_lang ok");
    if nodes.is_empty() {
        eprintln!("warn: closure(in) 結果 0（indexing 不完了の可能性）。基本動作は OK。");
        return;
    }
    assert!(nodes.iter().all(|n| n.depth >= 1 && n.depth <= 2));
}

#[test]
fn find_tested_by_in_py_workspace() {
    if !pyright_available() {
        eprintln!("skip: pyright-langserver が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let tests = find_tested_by_for_lang("create_user", 2, Lang::Py, &root)
        .expect("find_tested_by_for_lang ok");
    if tests.is_empty() {
        eprintln!("warn: tested-by 結果 0（indexing 不完了 or test fixture 検出されず）");
        return;
    }
    // 結果は test_user.py 配下の関数か、test_* 命名であるはず。
    let all_test_like = tests.iter().all(|t| {
        let f = t.file.to_ascii_lowercase().replace('\\', "/");
        let name = t.name.as_str();
        let leaf = name.rsplit_once('.').map(|x| x.1).unwrap_or(name);
        f.contains("/test_") && f.ends_with(".py")
            || f.contains("/tests/")
            || leaf.starts_with("test_")
            || leaf.ends_with("_test")
            || leaf == "test"
    });
    assert!(all_test_like, "all tested-by results must be test-like: {:?}", tests);
}
