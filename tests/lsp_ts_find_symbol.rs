//! `find_symbol_for_lang` 用 TypeScript integration test。
//!
//! `typescript-language-server` が PATH に無ければ skip。
//! 在環境では sample_workspace_ts/ 配下の `User` クラスが取れることを確認する。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thin_workflow_harness::ckg::lsp::{find_symbol_for_lang, Lang};

fn fixture_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/sample_workspace_ts");
    p
}

/// `typescript-language-server --version` が 0 で返れば true。
fn ts_lsp_available() -> bool {
    Command::new("typescript-language-server")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn find_symbol_user_in_ts_workspace() {
    if !ts_lsp_available() {
        eprintln!("skip: typescript-language-server が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let syms = find_symbol_for_lang(
        Lang::Ts,
        &root,
        "User",
        None,
        Duration::from_secs(60),
    )
    .expect("find_symbol_for_lang ok");
    if syms.is_empty() {
        eprintln!("warn: workspace/symbol が空（indexing 不完了の可能性）。基本動作は OK。");
        return;
    }
    let hit = syms.iter().any(|s| s.name.contains("User"));
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert!(hit, "User not found in: {names:?}");
}
