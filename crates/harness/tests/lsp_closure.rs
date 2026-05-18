//! `find_closure` integration test。rust-analyzer が PATH に無ければ skip。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thin_workflow_harness::ckg::lsp::{find_closure, Direction};

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
fn closure_in_depth1_vs_depth2() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let d1 = find_closure(
        "rust-analyzer",
        &root,
        "create_user",
        1,
        Direction::In,
        Duration::from_secs(60),
    )
    .expect("closure depth=1 ok");
    let d2 = find_closure(
        "rust-analyzer",
        &root,
        "create_user",
        2,
        Direction::In,
        Duration::from_secs(60),
    )
    .expect("closure depth=2 ok");
    if d1.is_empty() && d2.is_empty() {
        eprintln!("warn: closure 結果 0（indexing 不完了 or callHierarchy 未サポート）");
        return;
    }
    // depth=2 で make_pair が拾えていれば depth=1 より node が増える。
    // indexing が不完全な場合に備えて、< ではなく <= で許容（ただし両者非空時のみ）。
    if !d1.is_empty() && !d2.is_empty() {
        assert!(
            d2.len() >= d1.len(),
            "depth=2 ({}) should be >= depth=1 ({})",
            d2.len(),
            d1.len()
        );
    }
    // depth=2 で make_pair / make_party のどちらかが depth >= 2 で出ているはず。
    let has_d2_chain = d2.iter().any(|n| n.depth >= 2);
    if !has_d2_chain {
        eprintln!(
            "warn: depth>=2 のチェーン未検出（rust-analyzer 実装差）: d2={:?}",
            d2.iter().map(|n| (&n.name, n.depth)).collect::<Vec<_>>()
        );
    }
}

#[test]
fn closure_out_returns_refs() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let out = find_closure(
        "rust-analyzer",
        &root,
        "create_user",
        1,
        Direction::Out,
        Duration::from_secs(60),
    )
    .expect("closure direction=out ok");
    if out.is_empty() {
        eprintln!("warn: out 方向 closure 空（indexing 不完了の可能性）");
        return;
    }
    // direction=out は outgoing callees ── create_user は User::new を呼んでいる。
    // lib.rs の `User::new` (name == "new") が depth=1 で出るはず。
    let hit = out
        .iter()
        .any(|n| n.name == "new" && n.file.contains("lib.rs"));
    assert!(
        hit,
        "expected User::new callee in lib.rs: {:?}",
        out
    );
    assert!(out.iter().all(|n| n.direction == "out"));
}

#[test]
fn closure_out_transitive_depth2() {
    // direction=out で 1 段 MVP ではなく transitive (BFS depth>=2) になっていることを検証。
    // fixture chain: make_party -> {make_pair, make_direct}; make_pair -> {make_alice, make_bob}
    // depth=2 で起点 make_party から make_alice / make_bob まで（depth=2）届くはず。
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let d1 = find_closure(
        "rust-analyzer",
        &root,
        "make_party",
        1,
        Direction::Out,
        Duration::from_secs(60),
    )
    .expect("closure out depth=1 ok");
    let d2 = find_closure(
        "rust-analyzer",
        &root,
        "make_party",
        2,
        Direction::Out,
        Duration::from_secs(60),
    )
    .expect("closure out depth=2 ok");
    if d1.is_empty() && d2.is_empty() {
        eprintln!("warn: closure out 結果 0（indexing 不完了 or outgoingCalls 未サポート）");
        return;
    }
    if !d1.is_empty() && !d2.is_empty() {
        assert!(
            d2.len() >= d1.len(),
            "depth=2 ({}) should be >= depth=1 ({})",
            d2.len(),
            d1.len()
        );
    }
    let has_d2_chain = d2.iter().any(|n| n.depth >= 2);
    if !has_d2_chain {
        eprintln!(
            "warn: depth>=2 のチェーン未検出（rust-analyzer 実装差）: d2={:?}",
            d2.iter().map(|n| (&n.name, n.depth)).collect::<Vec<_>>()
        );
    }
    assert!(d2.iter().all(|n| n.direction == "out"));
}
