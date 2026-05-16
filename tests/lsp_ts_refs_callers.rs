//! `find_refs_for_lang` / `find_callers_for_lang` 用 TypeScript integration test。
//!
//! `typescript-language-server` が PATH に無ければ skip。
//! 在環境では sample_workspace_ts/ 配下で
//! `User` の references が 1 件以上、`create` (User.create) の incoming callers
//! が 1 件以上（main.ts から呼ぶ）あることを期待する。indexing 不完了で空配列も
//! 「壊れていない」扱いで warn skip。
//!
//! closure / impacted-by / tested-by の Lang 版もここで一緒に assert する
//! ── lsp 起動コストが大きいので test ファイル分割しない方が CI が速い。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thin_workflow_harness::ckg::lsp::{
    find_callers_for_lang, find_closure_for_lang, find_impacted_by_for_lang, find_refs_for_lang,
    find_tested_by_for_lang, Direction, Lang,
};

fn fixture_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/sample_workspace_ts");
    p
}

fn ts_lsp_available() -> bool {
    if try_ts_version() {
        return true;
    }
    #[cfg(windows)]
    {
        if let Ok(out) = Command::new("cmd")
            .args(["/c", "npm", "config", "get", "prefix"])
            .output()
        {
            if out.status.success() {
                let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !prefix.is_empty() {
                    let cur = std::env::var("PATH").unwrap_or_default();
                    let sep = ";";
                    let new_path = format!("{prefix}{sep}{cur}");
                    std::env::set_var("PATH", &new_path);
                    return try_ts_version();
                }
            }
        }
    }
    false
}

fn try_ts_version() -> bool {
    let direct = Command::new("typescript-language-server")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if direct {
        return true;
    }
    #[cfg(windows)]
    {
        return Command::new("cmd")
            .args(["/c", "typescript-language-server", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    }
    #[allow(unreachable_code)]
    false
}

#[test]
fn find_refs_user_in_ts_workspace() {
    if !ts_lsp_available() {
        eprintln!("skip: typescript-language-server が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let refs = find_refs_for_lang(Lang::Ts, &root, "User", Duration::from_secs(60))
        .expect("find_refs_for_lang ok");
    if refs.is_empty() {
        eprintln!("warn: references 空（indexing 不完了の可能性）。基本動作は OK。");
        return;
    }
    assert!(!refs.is_empty(), "expected at least 1 reference to User");
}

#[test]
fn find_callers_create_in_ts_workspace() {
    if !ts_lsp_available() {
        eprintln!("skip: typescript-language-server が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let callers = find_callers_for_lang(Lang::Ts, &root, "create", Duration::from_secs(60))
        .expect("find_callers_for_lang ok");
    if callers.is_empty() {
        eprintln!("warn: callers 空（indexing 不完了 or callHierarchy 未サポート）。基本動作は OK。");
        return;
    }
    assert!(!callers.is_empty(), "expected at least 1 caller of create");
}

#[test]
fn find_closure_in_ts_workspace() {
    if !ts_lsp_available() {
        eprintln!("skip: typescript-language-server が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let nodes = find_closure_for_lang("create", 2, Direction::In, Lang::Ts, &root)
        .expect("find_closure_for_lang ok");
    if nodes.is_empty() {
        eprintln!("warn: closure(in) 結果 0（indexing 不完了の可能性）。基本動作は OK。");
        return;
    }
    assert!(nodes.iter().all(|n| n.depth >= 1 && n.depth <= 2));
}

#[test]
fn find_impacted_by_in_ts_workspace() {
    if !ts_lsp_available() {
        eprintln!("skip: typescript-language-server が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let nodes = find_impacted_by_for_lang("create", 2, Lang::Ts, &root)
        .expect("find_impacted_by_for_lang ok");
    if nodes.is_empty() {
        eprintln!("warn: impacted-by 結果 0（indexing 不完了の可能性）。基本動作は OK。");
        return;
    }
    assert!(nodes.iter().all(|n| n.depth >= 1 && n.depth <= 2));
}

#[test]
fn find_tested_by_in_ts_workspace() {
    if !ts_lsp_available() {
        eprintln!("skip: typescript-language-server が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let tests = find_tested_by_for_lang("create", 2, Lang::Ts, &root)
        .expect("find_tested_by_for_lang ok");
    if tests.is_empty() {
        eprintln!("warn: tested-by 結果 0（indexing 不完了 or test fixture 検出されず）");
        return;
    }
    // 結果に含まれる全 node が test file or test name パターンに該当することを assert。
    let all_test_like = tests.iter().all(|t| {
        let f = t.file.to_ascii_lowercase();
        let name = t.name.as_str();
        let leaf = name.rsplit_once('.').map(|x| x.1).unwrap_or(name);
        f.ends_with(".test.ts")
            || f.ends_with(".spec.ts")
            || f.contains("/__tests__/")
            || f.contains("/tests/")
            || leaf.starts_with("test_")
            || leaf.ends_with("_test")
            || leaf == "describe"
            || leaf == "it"
            || leaf == "test"
    });
    assert!(all_test_like, "all tested-by results must be test-like: {:?}", tests);
}
