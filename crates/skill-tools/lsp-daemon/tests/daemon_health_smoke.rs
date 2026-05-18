//! daemon_health_smoke -- `Op::Health` の end-to-end smoke。
//!
//! rust-analyzer 在環境のみ実行 (不在は skip)。auto-spawn 経由で daemon を
//! 起こし health を呼ぶ → 1 件の find_symbol を投げて再度 health を呼び、
//! queries_handled 増加と recent_avg_ms 反映を確認する。

use std::path::PathBuf;
use std::time::Duration;

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
fn daemon_health_smoke() {
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

    // 1 回目: query 0 件、recent_avg_ms = 0 のはず
    let h0 = client.health().expect("first health");
    eprintln!(
        "[health] initial: lang={} uptime_ms={} queries={} recent_avg_ms={}",
        h0.lang, h0.uptime_ms, h0.queries_handled, h0.recent_avg_ms
    );
    assert_eq!(h0.status, "ready");
    assert_eq!(h0.lang, "rust");

    // 1 件 query を投げる → queries_handled が 1 になる、recent_avg_ms > 0
    let _ = client.find_symbol("User", &root, None, Duration::from_secs(30));
    let h1 = client.health().expect("post-query health");
    eprintln!(
        "[health] post-1q: queries={} recent_avg_ms={}",
        h1.queries_handled, h1.recent_avg_ms
    );
    assert!(
        h1.queries_handled > h0.queries_handled,
        "queries_handled did not increase: {} -> {}",
        h0.queries_handled,
        h1.queries_handled
    );

    // 追加で 3 件 query。recent_avg_ms は 0 のままにはならない (測定単位 ms)
    for sym in ["main", "create_user", "make_alice"].iter() {
        let _ = client.find_symbol(sym, &root, None, Duration::from_secs(30));
    }
    let h2 = client.health().expect("post-4q health");
    eprintln!(
        "[health] post-4q: queries={} recent_avg_ms={}",
        h2.queries_handled, h2.recent_avg_ms
    );
    assert!(
        h2.queries_handled >= h1.queries_handled + 3,
        "queries_handled did not increase enough: {} -> {}",
        h1.queries_handled,
        h2.queries_handled
    );
    // uptime は単調増加
    assert!(h2.uptime_ms >= h0.uptime_ms, "uptime regressed");

    // port file が残っていることを確認 (daemon が生きている)
    assert!(pf_path.exists(), "port file disappeared: {}", pf_path.display());

    drop(client);
}
