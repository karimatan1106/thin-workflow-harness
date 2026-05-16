//! LSP client 再利用 PoC ベンチ。1 client で複数 query を回し、per-query
//! レイテンシを実測する。per-invocation spawn を amortize すれば hot path
//! が < 1s に収まるかの実証 (layer 2.5)。
//!
//! rust-analyzer が PATH に無い環境は skip。assert は緩く、生数値は
//! --nocapture で stderr に流して人間が読む前提。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use thin_workflow_harness::ckg::lsp::{
    find_refs_for_lang_with_client, find_symbol_for_lang_with_client, start_and_warm_up, Lang,
};

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
fn bench_find_symbol_with_shared_client() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer not in PATH");
        return;
    }
    let root = fixture_root();

    let warm_start = Instant::now();
    let mut client = start_and_warm_up(Lang::Rust, &root).expect("warm-up failed");
    let warm_ms = warm_start.elapsed().as_millis();
    eprintln!("[bench] warm-up: {warm_ms}ms");

    let symbols = [
        "User",
        "create_user",
        "make_alice",
        "make_bob",
        "make_pair",
        "make_party",
        "make_direct",
        "build_inline_user",
        "MAX_USERS",
        "NotARealSymbol_zzz",
    ];
    let mut durations: Vec<u128> = Vec::new();
    for (i, sym) in symbols.iter().enumerate() {
        let t = Instant::now();
        let r = find_symbol_for_lang_with_client(
            &mut client,
            Lang::Rust,
            &root,
            sym,
            None,
            Duration::from_secs(60),
        );
        let ms = t.elapsed().as_millis();
        durations.push(ms);
        eprintln!(
            "[bench] query {} ({}): {}ms (result={})",
            i + 1,
            sym,
            ms,
            r.as_ref().map(|v| v.len()).unwrap_or(0)
        );
    }

    let total: u128 = durations.iter().sum();
    let avg = total / durations.len() as u128;
    let first = durations[0];
    let rest_avg = durations[1..].iter().sum::<u128>() / (durations.len() - 1) as u128;
    eprintln!(
        "[bench] find_symbol total={}ms avg={}ms first={}ms rest_avg={}ms",
        total, avg, first, rest_avg
    );

    let _ = client.shutdown();
    assert!(avg < 60_000, "per-query avg too slow: {}ms", avg);
}

#[test]
fn bench_find_refs_with_shared_client() {
    if !rust_analyzer_available() {
        eprintln!("skip: rust-analyzer not in PATH");
        return;
    }
    let root = fixture_root();

    let warm_start = Instant::now();
    let mut client = start_and_warm_up(Lang::Rust, &root).expect("warm-up failed");
    let warm_ms = warm_start.elapsed().as_millis();
    eprintln!("[bench-refs] warm-up: {warm_ms}ms");

    let mut durations: Vec<u128> = Vec::new();
    for i in 0..5 {
        let t = Instant::now();
        let r = find_refs_for_lang_with_client(
            &mut client,
            Lang::Rust,
            &root,
            "create_user",
            Duration::from_secs(60),
        );
        let ms = t.elapsed().as_millis();
        durations.push(ms);
        eprintln!(
            "[bench-refs] iter {}: {}ms (refs={})",
            i + 1,
            ms,
            r.as_ref().map(|v| v.len()).unwrap_or(0)
        );
    }
    let total: u128 = durations.iter().sum();
    let avg = total / durations.len() as u128;
    let first = durations[0];
    let rest_avg = durations[1..].iter().sum::<u128>() / (durations.len() - 1) as u128;
    eprintln!(
        "[bench-refs] total={}ms avg={}ms first={}ms rest_avg={}ms",
        total, avg, first, rest_avg
    );

    let _ = client.shutdown();
    assert!(avg < 60_000, "per-refs avg too slow: {}ms", avg);
}
