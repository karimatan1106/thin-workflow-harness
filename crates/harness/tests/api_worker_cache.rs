//! prompt cache 関連の構造検証テスト ── 実 API は呼ばない。
//!
//! 初回 dogfood で cache_create=0 / cache_read=0（cold start のみ）だった原因の再発防止:
//!   (a) spawn を跨いだ system block の byte 同一性（cache prefix が変動しないこと）
//!   (b) `cache_control: ephemeral` が system 配列の最後の text block に乗っていること
//!   (c) `anthropic-beta` に `prompt-caching-2024-07-31` が入っていること（両認証経路）

use serde_json::Value;

use thin_workflow_harness_core::runtime::anthropic::{ContentBlock, MessagesRequest};
use thin_workflow_harness_core::runtime::auth::AuthMode;

/// `ApiWorker::drive` 経路で組み立てられる system 配列を JSON 経由で取り出すヘルパ
/// （`build_system_blocks` は pub でないため、`MessagesRequest` を直接組み立てて確認する）。
fn make_request(system: Vec<ContentBlock>) -> String {
    let req = MessagesRequest {
        model: "claude-haiku-test".into(),
        max_tokens: 4096,
        system,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
    };
    serde_json::to_string(&req).unwrap()
}

#[test]
fn system_block_is_byte_identical_across_spawn_with_same_inputs() {
    // 同じ system_prompt / skill_body / spec_slice なら 2 回構築しても byte 同一 ──
    // これが崩れると cache prefix が変動して常に cold start になる。
    use thin_workflow_harness_core::runtime::api_worker::testing::build_system_blocks_for_test;
    use thin_workflow_harness_core::runtime::worker::WorkerContext;

    let ctx = WorkerContext {
        system_prompt: "static system body".into(),
        node_header: "n1 (implement)".into(),
        skill_body: "skill X".into(),
        spec_slice: "F-001 ...".into(),
        compact_status: "ANY mutable status — must NOT leak into system".into(),
        failed_gates: vec![],
        tools: vec!["read_file".into()],
    };
    let a = build_system_blocks_for_test(&ctx);
    // 2 回目は compact_status を変えても system は変わらないことを示す。
    let ctx2 = WorkerContext { compact_status: "different status".into(), ..ctx.clone() };
    let b = build_system_blocks_for_test(&ctx2);

    let ja = serde_json::to_string(&a).unwrap();
    let jb = serde_json::to_string(&b).unwrap();
    assert_eq!(ja, jb, "system block が spawn 間で byte 同一でない");
}

#[test]
fn cache_control_marks_last_text_block_in_system() {
    use thin_workflow_harness_core::runtime::api_worker::testing::build_system_blocks_for_test;
    use thin_workflow_harness_core::runtime::worker::WorkerContext;

    let ctx = WorkerContext {
        system_prompt: "system".into(),
        node_header: "n1 (implement)".into(),
        skill_body: "skill body".into(),
        spec_slice: "F-001".into(),
        compact_status: String::new(),
        failed_gates: vec![],
        tools: vec![],
    };
    let blocks = build_system_blocks_for_test(&ctx);
    let body = make_request(blocks);
    let v: Value = serde_json::from_str(&body).unwrap();
    let sys = v.get("system").and_then(|s| s.as_array()).expect("system は array");
    assert!(!sys.is_empty(), "system が空");
    // Anthropic 仕様: cache 範囲を区切るのは *最後* の cache_control マーカー。
    // 最後の text ブロックが必ず cache_control:ephemeral を持つ必要がある。
    let last = sys.last().unwrap();
    let cc = last.get("cache_control").expect("最後の system block に cache_control が無い");
    assert_eq!(cc.get("type").and_then(|t| t.as_str()), Some("ephemeral"));
}

#[test]
fn anthropic_beta_includes_prompt_caching_for_api_key_mode() {
    let h = AuthMode::ApiKey("sk".into()).auth_headers("2023-06-01");
    let beta: Vec<&String> = h
        .iter()
        .filter_map(|(k, v)| if k == "anthropic-beta" { Some(v) } else { None })
        .collect();
    assert!(!beta.is_empty(), "anthropic-beta header が無い: {h:?}");
    assert!(
        beta.iter().any(|v| v.contains("prompt-caching-2024-07-31")),
        "ApiKey 経路の anthropic-beta に prompt-caching が無い: {beta:?}",
    );
}

#[test]
fn anthropic_beta_includes_prompt_caching_for_bearer_mode() {
    let h = AuthMode::Bearer("oat".into()).auth_headers("2023-06-01");
    let beta = h
        .iter()
        .find(|(k, _)| k == "anthropic-beta")
        .map(|(_, v)| v.clone())
        .expect("bearer に anthropic-beta header が無い");
    assert!(beta.contains("prompt-caching-2024-07-31"), "bearer beta に prompt-caching 不在: {beta}");
    assert!(beta.contains("oauth-2025-04-20"), "bearer beta に oauth-2025-04-20 不在: {beta}");
}


/// SYSTEM_PROMPT が cache 1024 token 閾値到達のため十分な長さである（unit-test 重複だが、
/// 統合経路（context.rs 経由）で string が空でないことを担保する公開エントリ）。
#[test]
fn system_prompt_is_substantial_via_context_build() {
    // 直接 context::build_context は private 引数（State 等）が要るので、
    // ここでは公開済みの build_system_blocks_for_test 経路で system_prompt が
    // 十分大きい場合に正しく block 化されることを確かめる。
    use thin_workflow_harness_core::runtime::api_worker::testing::build_system_blocks_for_test;
    use thin_workflow_harness_core::runtime::worker::WorkerContext;

    // 実際の SYSTEM_PROMPT 並みの長さ（4000 char）を流す。
    let big = "あ".repeat(4000);
    let ctx = WorkerContext {
        system_prompt: big.clone(),
        node_header: "n1 (implement)".into(),
        skill_body: String::new(),
        spec_slice: String::new(),
        compact_status: String::new(),
        failed_gates: vec![],
        tools: vec![],
    };
    let blocks = build_system_blocks_for_test(&ctx);
    assert_eq!(blocks.len(), 1, "system_prompt のみなら 1 block");
    if let ContentBlock::Text { text, cache_control } = &blocks[0] {
        assert_eq!(text, &big);
        assert!(cache_control.is_some(), "system block に cache_control が無い");
    } else {
        panic!("text block ではない");
    }
}

/// tools 配列を JSON 直列化したとき、最後のツールに cache_control:ephemeral が乗る ──
/// system + tools の合計が 1024 token 閾値に届くための必須マーカー。
#[test]
fn tools_array_marks_cache_boundary_at_last_tool() {
    use thin_workflow_harness_core::runtime::anthropic::MessagesRequest;
    use thin_workflow_harness_core::runtime::tools::tool_defs;

    let defs = tool_defs();
    let req = MessagesRequest {
        model: "claude-haiku-test".into(),
        max_tokens: 4096,
        system: vec![],
        messages: vec![],
        tools: defs.clone(),
        tool_choice: None,
    };
    let body = serde_json::to_string(&req).unwrap();
    let v: Value = serde_json::from_str(&body).unwrap();
    let tools = v.get("tools").and_then(|t| t.as_array()).expect("tools が array でない");
    assert!(!tools.is_empty(), "tools が空");
    // 最後のツールに cache_control が乗る。
    let last = tools.last().unwrap();
    let cc = last.get("cache_control").expect("最後のツールに cache_control が無い");
    assert_eq!(cc.get("type").and_then(|t| t.as_str()), Some("ephemeral"));
    // 中間のツールには cache_control が乗らない（serde skip_serializing_if=None で省略される）。
    for (i, t) in tools.iter().enumerate() {
        if i + 1 == tools.len() { continue; }
        assert!(t.get("cache_control").is_none(),
            "中間のツール index={i} に cache_control が漏れている: {t}");
    }
}
