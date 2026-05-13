//! prompt cache 関連の構造検証テスト ── 実 API は呼ばない。
//!
//! 初回 dogfood で cache_create=0 / cache_read=0（cold start のみ）だった原因の再発防止:
//!   (a) spawn を跨いだ system block の byte 同一性（cache prefix が変動しないこと）
//!   (b) `cache_control: ephemeral` が system 配列の最後の text block に乗っていること
//!   (c) `anthropic-beta` に `prompt-caching-2024-07-31` が入っていること（両認証経路）

use serde_json::Value;

use thin_workflow_harness::runtime::anthropic::{ContentBlock, MessagesRequest};
use thin_workflow_harness::runtime::auth::AuthMode;

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
    use thin_workflow_harness::runtime::api_worker::testing::build_system_blocks_for_test;
    use thin_workflow_harness::runtime::worker::WorkerContext;

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
    use thin_workflow_harness::runtime::api_worker::testing::build_system_blocks_for_test;
    use thin_workflow_harness::runtime::worker::WorkerContext;

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
