//! daemon_smoke -- foreground daemon の end-to-end ベンチ。
//!
//! rust-analyzer 在環境のみ。daemon を別 thread で起動し、TCP client から
//! 連続 find_symbol を投げて hot path が sub-second に収まるかを実測する。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use thin_workflow_harness::ckg::lsp::Lang;
use thin_workflow_harness::lsp_daemon::{run_daemon, DaemonClient};

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

/// listener.local_addr() で port を得るので、ここは bind 候補 (0=OS割当) を返す。
fn pick_port() -> u16 {
    0
}

/// 起動済みの daemon port を読み取る薄いリスナ ─ 実体は run_daemon 内で
/// stderr に [daemon] line を吐くが、テストでは「先に TCP bind してその port を
/// 渡す」方式で OS-assign 後の port を取得しないため、ここでは固定の高 port を試す。
/// 失敗したら別 port にリトライする戦略を取る。
fn try_free_port() -> u16 {
    // 49152-65535 のうち適当な範囲を試す
    use std::net::TcpListener;
    for p in 49500u16..49600u16 {
        if TcpListener::bind(("127.0.0.1", p)).is_ok() {
            return p;
        }
    }
    0
}

#[test]
fn daemon_find_symbol_reuse_bench() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer not in PATH");
        return;
    }
    let _ = pick_port();
    let root = fixture_root();
    let port = try_free_port();
    if port == 0 {
        eprintln!("skip: no free port in test range");
        return;
    }
    let root2 = root.clone();
    let _daemon = std::thread::spawn(move || {
        let _ = run_daemon(Lang::Rust, root2, port);
    });
    // daemon 起動 + warm-up を待つ。connect retry でカバー。
    let connect_deadline = Instant::now() + Duration::from_secs(60);
    let mut client = loop {
        match DaemonClient::connect(port) {
            Ok(c) => break c,
            Err(_) if Instant::now() < connect_deadline => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => panic!("connect failed: {e}"),
        }
    };

    let symbols = ["User", "create_user", "main", "make_alice", "MAX_USERS"];
    let mut durations: Vec<u128> = Vec::new();
    for sym in symbols.iter() {
        let t = Instant::now();
        let r = client.find_symbol(sym, &root, None, Duration::from_secs(30));
        let ms = t.elapsed().as_millis();
        durations.push(ms);
        eprintln!(
            "[daemon-bench] {}: {}ms ({:?})",
            sym,
            ms,
            r.as_ref().map(|v| v.len())
        );
    }
    let rest_avg: u128 =
        durations[1..].iter().sum::<u128>() / (durations.len() - 1) as u128;
    eprintln!("[daemon-bench] rest_avg: {}ms", rest_avg);
    // 1 client reuse 想定なので sub-second 余裕で出るはず。500ms 上限で assert。
    assert!(rest_avg < 500, "rest_avg too slow: {}ms", rest_avg);
    // daemon thread は test 終了で leak されて OK (OS が回収)。
}

#[test]
fn daemon_unknown_symbol_returns_empty() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer not in PATH");
        return;
    }
    let root = fixture_root();
    let port = try_free_port();
    if port == 0 {
        eprintln!("skip: no free port in test range");
        return;
    }
    let root2 = root.clone();
    let _daemon = std::thread::spawn(move || {
        let _ = run_daemon(Lang::Rust, root2, port);
    });
    let connect_deadline = Instant::now() + Duration::from_secs(60);
    let mut client = loop {
        match DaemonClient::connect(port) {
            Ok(c) => break c,
            Err(_) if Instant::now() < connect_deadline => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => panic!("connect failed: {e}"),
        }
    };
    let r = client
        .find_symbol("ZzNotReal999", &root, None, Duration::from_secs(30))
        .expect("should not error on miss");
    assert!(r.is_empty(), "expected empty for unknown symbol, got {:?}", r);
}
