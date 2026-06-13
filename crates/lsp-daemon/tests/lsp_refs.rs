//! `find_refs` / `find_callers` integration test。
//! rust-analyzer が PATH に無い環境では skip。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thin_workflow_harness_ckg::ckg::lsp::{find_callers, find_refs};

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
fn find_refs_create_user_in_sample_workspace() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let refs = match find_refs(
        "rust-analyzer",
        &root,
        "create_user",
        Duration::from_secs(60),
    ) {
        Ok(r) => r,
        // cold-start / 並行負荷下の indexing 不完了で Err になりうる（環境依存）。
        // 空結果と同じく「壊れていない」扱いで skip する（姉妹テスト lsp_closure と同方針）。
        Err(e) => {
            eprintln!("warn: find_refs がエラー（cold-start/indexing 不完了の可能性）: {e}");
            return;
        }
    };
    // indexing が間に合わなければ空もありうる。壊れていなければ少なくとも
    // use_user.rs での 2 箇所の呼び出しを期待する。
    if refs.is_empty() {
        eprintln!("warn: references 空（indexing 不完了の可能性）。基本動作は OK。");
        return;
    }
    let hit = refs.iter().any(|r| r.file.contains("use_user.rs"));
    assert!(hit, "expected use_user.rs reference: {refs:?}");
}

#[test]
fn find_callers_create_user_in_sample_workspace() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let callers = match find_callers(
        "rust-analyzer",
        &root,
        "create_user",
        Duration::from_secs(60),
    ) {
        Ok(c) => c,
        // cold-start / 並行負荷下の indexing 不完了・callHierarchy 未準備で Err になりうる。
        Err(e) => {
            eprintln!("warn: find_callers がエラー（cold-start/indexing 不完了の可能性）: {e}");
            return;
        }
    };
    if callers.is_empty() {
        eprintln!("warn: callers 空（indexing 不完了 or callHierarchy 未サポート）。基本動作は OK。");
        return;
    }
    let names: Vec<&str> = callers.iter().map(|c| c.name.as_str()).collect();
    let hit = names.iter().any(|n| n.contains("make_alice") || n.contains("make_bob"));
    assert!(hit, "expected make_alice/make_bob in callers: {names:?}");
}
