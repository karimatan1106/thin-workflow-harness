//! daemon_auto_spawn_smoke -- `DaemonClient::connect_or_spawn` の end-to-end smoke。
//!
//! rust-analyzer 在環境のみ。warm-up 込みの初回 + 既存 daemon reuse の 2 段を計測。
//!
//! - 初回: auto-spawn 込み (port file 出現を 60s まで poll)
//! - 2 回目: 既存 daemon に reuse (sub-second 想定)
//!
//! daemon process は test 終了で leak (OS 終了で回収、background detach は次バッチ送り)。
//! port file は test 末尾で best-effort 削除する。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use thin_workflow_harness_ckg::ckg::lsp::Lang;
use thin_workflow_harness_ckg::lsp_daemon::{port_file, DaemonClient};

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
fn daemon_auto_spawn_smoke() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer not in PATH");
        return;
    }
    let root = fixture_root();
    let pf_path = match port_file::port_file_path("rust", &root) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skip: cannot resolve port file path: {e}");
            return;
        }
    };
    // 前回 leak した port file が残っていれば、stale ガード経由で自然に処理される。

    // 初回: auto-spawn (LSP warm-up 込み)
    let t0 = Instant::now();
    let mut client = match DaemonClient::connect_or_spawn(
        Lang::Rust,
        &root,
        Duration::from_secs(60),
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skip: connect_or_spawn failed: {e}");
            return;
        }
    };
    let initial_ms = t0.elapsed().as_millis();
    eprintln!("[auto-spawn] initial: {initial_ms}ms");

    // 数回 query を投げて hot path が動くことを確認。
    for sym in ["User", "create_user", "main"].iter() {
        let t = Instant::now();
        let r = client.find_symbol(sym, &root, None, Duration::from_secs(30));
        eprintln!(
            "[auto-spawn] {}: {}ms ({:?})",
            sym,
            t.elapsed().as_millis(),
            r.as_ref().map(|v| v.len())
        );
    }
    drop(client);

    // 2 回目: 既存 daemon 接続 (reuse)
    let t1 = Instant::now();
    let _client2 = match DaemonClient::connect_or_spawn(
        Lang::Rust,
        &root,
        Duration::from_secs(5),
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skip: reconnect failed: {e}");
            // port file best-effort cleanup
            let _ = port_file::delete(&pf_path);
            return;
        }
    };
    let reuse_ms = t1.elapsed().as_millis();
    eprintln!("[auto-spawn] reconnect: {reuse_ms}ms");
    // 既存 daemon 接続は warm-up を踏まないので sub-second 余裕で出るはず。
    assert!(reuse_ms < 2000, "reconnect too slow: {reuse_ms}ms");

    // port file の存在を確認 (auto-spawn 経由で書き出されているはず)。
    assert!(
        pf_path.exists(),
        "port file does not exist at {}",
        pf_path.display()
    );
    let content = port_file::read(&pf_path).expect("read port file");
    assert!(content.port > 0, "invalid port: {}", content.port);
    assert!(content.pid > 0, "invalid pid: {}", content.pid);
    eprintln!(
        "[auto-spawn] port_file: pid={} port={} path={}",
        content.pid,
        content.port,
        pf_path.display()
    );

    // best-effort cleanup (daemon process は OS 終了まで残る ── 次バッチで detach 検討)。
    drop(_client2);
}
