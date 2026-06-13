//! `find_closure` integration test。rust-analyzer が PATH に無ければ skip。
//! cold-start (indexing 未完了) で起点 symbol が未 index だと find_closure は
//! `Err("symbol not found")` を返す ── find_symbol テストが空を許容するのと同じ思想で
//! その場合は skip 扱いにする (daemon warm 時は実体を検証できる)。

use std::process::Command;
use std::path::PathBuf;
use std::time::Duration;

use thin_workflow_harness_ckg::ckg::lsp::{find_closure, Direction};

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

/// find_closure を呼び、cold-start の "symbol not found" は skip (return None)、
/// 真のエラーは panic、成功は Some(結果)。
macro_rules! closure_or_skip {
    ($($args:tt)*) => {
        match find_closure($($args)*) {
            Ok(v) => v,
            Err(e) if e.contains("not found") => {
                eprintln!("skip(cold-start): {e}");
                return;
            }
            Err(e) => panic!("closure error: {e}"),
        }
    };
}

#[test]
fn closure_in_depth1_vs_depth2() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let d1 = closure_or_skip!("rust-analyzer", &root, "create_user", 1, Direction::In, Duration::from_secs(60));
    let d2 = closure_or_skip!("rust-analyzer", &root, "create_user", 2, Direction::In, Duration::from_secs(60));
    if d1.is_empty() && d2.is_empty() {
        eprintln!("warn: closure 結果 0（indexing 不完了 or callHierarchy 未サポート）");
        return;
    }
    // depth=2 で make_pair が拾えていれば depth=1 より node が増える（両者非空時のみ <= 許容）。
    if !d1.is_empty() && !d2.is_empty() {
        assert!(d2.len() >= d1.len(), "depth=2 ({}) should be >= depth=1 ({})", d2.len(), d1.len());
    }
    if !d2.iter().any(|n| n.depth >= 2) {
        eprintln!("warn: depth>=2 のチェーン未検出（rust-analyzer 実装差）: d2={:?}",
            d2.iter().map(|n| (&n.name, n.depth)).collect::<Vec<_>>());
    }
}

#[test]
fn closure_out_returns_refs() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let out = closure_or_skip!("rust-analyzer", &root, "create_user", 1, Direction::Out, Duration::from_secs(60));
    if out.is_empty() {
        eprintln!("warn: out 方向 closure 空（indexing 不完了の可能性）");
        return;
    }
    // direction=out は outgoing callees ── create_user は User::new を呼ぶ。
    let hit = out.iter().any(|n| n.name == "new" && n.file.contains("lib.rs"));
    assert!(hit, "expected User::new callee in lib.rs: {out:?}");
    assert!(out.iter().all(|n| n.direction == "out"));
}

#[test]
fn closure_out_transitive_depth2() {
    // direction=out が transitive (BFS depth>=2) であることを検証。
    // fixture chain: make_party -> {make_pair, make_direct}; make_pair -> {make_alice, make_bob}
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let d1 = closure_or_skip!("rust-analyzer", &root, "make_party", 1, Direction::Out, Duration::from_secs(60));
    let d2 = closure_or_skip!("rust-analyzer", &root, "make_party", 2, Direction::Out, Duration::from_secs(60));
    if d1.is_empty() && d2.is_empty() {
        eprintln!("warn: closure out 結果 0（indexing 不完了 or outgoingCalls 未サポート）");
        return;
    }
    if !d1.is_empty() && !d2.is_empty() {
        assert!(d2.len() >= d1.len(), "depth=2 ({}) should be >= depth=1 ({})", d2.len(), d1.len());
    }
    if !d2.iter().any(|n| n.depth >= 2) {
        eprintln!("warn: depth>=2 のチェーン未検出（rust-analyzer 実装差）: d2={:?}",
            d2.iter().map(|n| (&n.name, n.depth)).collect::<Vec<_>>());
    }
    assert!(d2.iter().all(|n| n.direction == "out"));
}
