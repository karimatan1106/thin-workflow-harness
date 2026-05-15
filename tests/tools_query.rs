//! `query_*` 系 tool の round-trip 検証 ── subprocess は実 harness.exe を呼ぶが
//! Anthropic API は呼ばない（HARNESS_BIN env で test binary を指定）。
//!
//! 検証範囲:
//! 1. `tool_use_to_call("query_outline", ...)` が `ToolCall::Query` を返す。
//! 2. `tools_query::run_query` が `HARNESS_BIN` で指定された subprocess を起動し、
//!    `outline` の stdout を取得できる（rust-analyzer 不在でも動く）。
//! 3. mock 経路: 1 ターンの assistant 応答に `query_outline` の tool_use が乗っていれば
//!    `apply_loop::run_tool_uses` 風の経路で subprocess を呼んで結果が tool_result に
//!    詰まる ── ここは `tool_use_to_call` + `run_query` の手動結線で確認する
//!    （実 ApiWorker drive は API キーが要るので scope 外）。

use std::path::{Path, PathBuf};

use serde_json::json;

use thin_workflow_harness::runtime::tools::{tool_use_to_call, ToolCall};
use thin_workflow_harness::runtime::tools_query::{build_query_spec, run_query, QuerySpec};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// テスト用に harness binary を test build に向ける。
/// `CARGO_BIN_EXE_harness` は cargo test が用意してくれる絶対パス。
fn set_harness_bin_for_tests() {
    std::env::set_var("HARNESS_BIN", env!("CARGO_BIN_EXE_harness"));
}

#[test]
fn query_outline_tool_use_maps_to_query_variant() {
    let c = tool_use_to_call("query_outline", &json!({"file":"src/lib.rs"})).unwrap();
    match c {
        ToolCall::Query(q) => {
            assert_eq!(q.subcommand, "outline");
            assert_eq!(q.positional, "src/lib.rs");
        }
        _ => panic!("query_outline は ToolCall::Query を返すべき"),
    }
}

#[test]
fn run_query_outline_against_real_harness_subprocess() {
    set_harness_bin_for_tests();
    let spec = QuerySpec {
        subcommand: "outline".into(),
        positional: "tests/fixtures/sample_workspace_rust/src/lib.rs".into(),
        depth: None, direction: None, root: None, format: None,
    };
    let out = run_query(&spec, &manifest_dir()).expect("run_query 起動");
    assert!(out.success, "outline subprocess が失敗: stderr={}", out.stderr);
    // sample_workspace_rust/src/lib.rs に含まれる top-level シンボルの 1 つ。
    assert!(out.stdout.contains("User") || out.stdout.contains("create_user"),
        "outline stdout に既知シンボルが無い: {}", out.stdout);
}

#[test]
fn run_query_unknown_subcommand_returns_error_subprocess() {
    set_harness_bin_for_tests();
    let spec = QuerySpec {
        subcommand: "definitely-not-a-subcommand".into(),
        positional: "x".into(),
        depth: None, direction: None, root: None, format: None,
    };
    let out = run_query(&spec, &manifest_dir()).expect("起動自体は成功する");
    assert!(!out.success, "未知 subcommand なら non-zero exit");
}

#[test]
fn mock_round_trip_query_outline_via_tool_use_to_call_then_run_query() {
    // mock シナリオ: assistant が tool_use(query_outline, {file: ...}) を返したと仮定し、
    // tool_use_to_call で正規化 → run_query で subprocess 起動 → stdout が取れる。
    set_harness_bin_for_tests();
    let tool_use_input = json!({"file": "tests/fixtures/sample_workspace_rust/src/lib.rs"});
    let call = tool_use_to_call("query_outline", &tool_use_input).unwrap();
    let spec = match call {
        ToolCall::Query(q) => q,
        _ => panic!("query_outline は ToolCall::Query"),
    };
    let out = run_query(&spec, &manifest_dir()).expect("run_query");
    assert!(out.success);
    assert!(!out.stdout.is_empty(), "outline は何かしら出力する");
}

#[test]
fn build_query_spec_hyphenates_impacted_by() {
    let q = build_query_spec("query_impacted_by", &json!({"qname":"foo"})).unwrap();
    assert_eq!(q.subcommand, "impacted-by");
}

#[test]
fn run_query_uses_provided_cwd_for_relative_paths() {
    set_harness_bin_for_tests();
    // 相対パスを使う ── cwd = manifest_dir なので tests/fixtures/... が解決できる。
    let spec = QuerySpec {
        subcommand: "outline".into(),
        positional: "tests/fixtures/sample_workspace_rust/src/lib.rs".into(),
        depth: None, direction: None, root: None, format: None,
    };
    let cwd: &Path = &manifest_dir();
    let out = run_query(&spec, cwd).expect("subprocess");
    assert!(out.success, "cwd 解決失敗: stderr={}", out.stderr);
}
