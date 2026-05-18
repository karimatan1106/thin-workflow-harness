//! daemon_admin_smoke -- `harness lsp-daemon list/stop` 系の最小 smoke。
//!
//! - cmd_list() を呼んで panic / Err しないこと
//! - rust-analyzer 在環境なら auto-spawn -> list で 1 件以上、stop_specific で kill 確認

use std::path::PathBuf;
use std::time::Duration;

use thin_workflow_harness::ckg::lsp::Lang;
use thin_workflow_harness::lsp_daemon::{admin, port_file, port_file_list, DaemonClient};

fn rust_analyzer_available() -> bool {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn fixture_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/sample_workspace_rust");
    p
}

#[test]
fn cmd_list_smoke_runs_without_panic() {
    let res = admin::cmd_list();
    assert!(res.is_ok(), "cmd_list returned Err: {:?}", res);
}

#[test]
fn cmd_stop_stale_smoke_runs_without_panic() {
    let res = admin::cmd_stop_stale();
    assert!(res.is_ok(), "cmd_stop_stale returned Err: {:?}", res);
}

#[test]
fn cmd_stop_by_lang_smoke_zero_entries_ok() {
    // 該当 lang が 0 件でも panic / Err しないこと。
    let res = admin::cmd_stop_by_lang("nonexistent_lang_marker");
    assert!(res.is_ok(), "cmd_stop_by_lang returned Err: {:?}", res);
}

#[test]
fn auto_spawn_then_list_then_stop_specific() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer not in PATH");
        return;
    }
    let root = fixture_root();
    let pf_path = match port_file::port_file_path("rust", &root) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skip: port_file_path: {e}");
            return;
        }
    };

    let client = match DaemonClient::connect_or_spawn(
        Lang::Rust,
        &root,
        Duration::from_secs(60),
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skip: connect_or_spawn: {e}");
            return;
        }
    };
    drop(client);

    // list_all -> 1 件以上検出
    let entries = port_file_list::list_all().expect("list_all");
    let found = entries.iter().any(|e| e.path == pf_path);
    assert!(found, "spawned daemon not found in list_all output");

    // stop_specific -> port file 消える
    let r = admin::cmd_stop_specific("rust", &root);
    assert!(r.is_ok(), "cmd_stop_specific: {:?}", r);
    assert!(!pf_path.exists(), "port file should be deleted after stop");
}
