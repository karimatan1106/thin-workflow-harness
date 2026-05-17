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
//! 4. lang 流通: `lang="ts"` を渡したとき build_query_spec が拾い、run_query が
//!    `--lang ts` を subprocess に渡すこと。lang 省略時は従来挙動と同じ。

use std::path::{Path, PathBuf};

use serde_json::json;

use thin_workflow_harness::runtime::tools::{tool_use_to_call, ToolCall};
use thin_workflow_harness::runtime::tools_query::{build_query_spec, run_query, QuerySpec};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// テスト用に harness binary を test build に向ける。
/// `CARGO_BIN_EXE_harness` は cargo test が用意してくれる絶対パス。
/// daemon default 化 (--use-daemon 撤去) 後は subprocess 側で daemon spawn が
/// 走ると 30s timeout / handle inherit でテストが hang する。テスト経路は
/// direct LSP 固定 (HARNESS_DIRECT_LSP=1) にして daemon を bypass する。
fn set_harness_bin_for_tests() {
    std::env::set_var("HARNESS_BIN", env!("CARGO_BIN_EXE_harness"));
    std::env::set_var("HARNESS_DIRECT_LSP", "1");
}

#[test]
fn query_outline_tool_use_maps_to_query_variant() {
    let c = tool_use_to_call("query_outline", &json!({"file":"src/lib.rs"})).unwrap();
    match c {
        ToolCall::Query(q) => {
            assert_eq!(q.subcommand, "outline");
            assert_eq!(q.positional, "src/lib.rs");
            // outline は CLI 側が --lang を受け付けないので tool_use_to_call は必ず None。
            assert!(q.lang.is_none(), "outline には lang を流さない");
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
        depth: None, direction: None, root: None, format: None, lang: None,
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
        depth: None, direction: None, root: None, format: None, lang: None,
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
        depth: None, direction: None, root: None, format: None, lang: None,
    };
    let cwd: &Path = &manifest_dir();
    let out = run_query(&spec, cwd).expect("subprocess");
    assert!(out.success, "cwd 解決失敗: stderr={}", out.stderr);
}

/// lang 流通の round-trip ── tool_use_to_call で lang="ts" を渡したとき、
/// QuerySpec.lang に伝播することを assert（schema は枠だけ提供する LLM 向けで、
/// 実 dispatch では build_query_spec が拾う ── そこを検証する）。
#[test]
fn query_symbol_tool_use_propagates_lang_to_spec() {
    let c = tool_use_to_call("query_symbol", &json!({"qname":"User.create", "lang":"ts"})).unwrap();
    match c {
        ToolCall::Query(q) => {
            assert_eq!(q.subcommand, "symbol");
            assert_eq!(q.lang.as_deref(), Some("ts"), "lang=ts が流通していない");
        }
        _ => panic!("query_symbol は ToolCall::Query"),
    }
}

/// run_query で lang を持つ spec を回したとき、subprocess 側が --lang を受理して
/// non-zero exit せずに stdout を返すこと（backward compat の確認も兼ねる）。
/// rust-analyzer/typescript-language-server がなくても outline は通るが、
/// 6 query は LSP 不在で stderr に詰まるので、ここでは stdout が呼べることだけを assert。
#[test]
fn run_query_appends_lang_arg_when_present() {
    set_harness_bin_for_tests();
    // 存在しないシンボルでもよい ── --lang フラグが args に乗って subprocess が起動できるか
    // だけを見たい。LSP 不在環境では stderr に "language server" 等が出るが起動はする。
    let spec = QuerySpec {
        subcommand: "symbol".into(),
        positional: "definitely_nonexistent_symbol_xyz".into(),
        depth: None, direction: None, root: None, format: Some("json".into()),
        lang: Some("rust".into()),
    };
    let out = run_query(&spec, &manifest_dir()).expect("subprocess 起動");
    // success/fail は LSP 有無に依存するので、stderr が `--lang` 自体の reject を
    // 出していないこと（"unexpected argument" 等を含まない）を確認するに留める。
    let combined = format!("{}{}", out.stdout, out.stderr);
    assert!(!combined.contains("unexpected argument"),
        "--lang フラグが CLI 側で reject された: {combined}");
    assert!(!combined.contains("invalid value for"),
        "--lang の値 'rust' が enum reject された: {combined}");
}

/// lang 省略 (= None) のときに run_query が --lang を一切 append しないこと。
/// 直接 args 観測はできないので、CLI 既定 auto と同等に subprocess が動くことを
/// 既存 outline test と同じパターンで担保する（backward compat スモーク）。
#[test]
fn run_query_without_lang_remains_backward_compatible() {
    set_harness_bin_for_tests();
    let spec = QuerySpec {
        subcommand: "outline".into(),
        positional: "tests/fixtures/sample_workspace_rust/src/lib.rs".into(),
        depth: None, direction: None, root: None, format: None, lang: None,
    };
    let out = run_query(&spec, &manifest_dir()).expect("run_query");
    assert!(out.success, "lang 省略時に outline が壊れている: stderr={}", out.stderr);
}
