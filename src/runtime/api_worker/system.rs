//! system block と初期 user メッセージの構築 ── `drive` から切り出した責務分離モジュール。
//!
//! ここで扱うのは「会話の入口」だけ:
//! - `build_system_blocks` ── prompt caching を効かせる 2 block 構成。
//! - `initial_user_message` ── ノードヘッダ + status + （respawn なら）failed_gates。
//!
//! 動的なターン回しは `drive.rs`、API リトライは `retry.rs`、ツール apply は `apply_loop.rs`。

use crate::runtime::anthropic::{CacheControl, ContentBlock, Message};
use crate::runtime::worker::WorkerContext;

/// system を「静的本文」「skill + spec スライス」の 2 ブロックに割り、両方に cache_control を付ける。
/// `cache_control: ephemeral` は最後の text block にも必ず付ける（Anthropic 仕様: 最後の
/// マーカー位置までが cache prefix）── これが無いと cache が hint されない。
pub(crate) fn build_system_blocks(ctx: &WorkerContext) -> Vec<ContentBlock> {
    let mut out = Vec::new();
    if !ctx.system_prompt.is_empty() {
        out.push(ContentBlock::Text {
            text: ctx.system_prompt.clone(),
            cache_control: Some(CacheControl::ephemeral()),
        });
    }
    let mut prefix = String::new();
    if !ctx.skill_body.is_empty() {
        prefix.push_str("# ノード skill\n");
        prefix.push_str(&ctx.skill_body);
        prefix.push('\n');
    }
    if !ctx.spec_slice.is_empty() {
        prefix.push_str("\n# spec スライス\n");
        prefix.push_str(&ctx.spec_slice);
        prefix.push('\n');
    }
    if !prefix.is_empty() {
        out.push(ContentBlock::Text {
            text: prefix,
            cache_control: Some(CacheControl::ephemeral()),
        });
    }
    out
}

/// 初期 user メッセージ ── ノードヘッダ + 可変サフィックス（status, feedback）。
pub(super) fn initial_user_message(ctx: &WorkerContext) -> Message {
    let mut text = String::new();
    text.push_str(&format!("# 現ノード\n{}\n\n", ctx.node_header));
    text.push_str("# 現在の status（harness 観測）\n");
    text.push_str(&ctx.compact_status);
    text.push('\n');
    if ctx.is_respawn() {
        text.push_str("\n# 直前 advance_rejected の failed_gates（再 spawn フィードバック）\n");
        for (g, r) in &ctx.failed_gates {
            text.push_str(&format!("- {g}: {r}\n"));
        }
    }
    text.push_str("\n# 渡されたツール\n");
    text.push_str(&ctx.tools.join(", "));
    text.push('\n');
    text.push_str("\n作業が完了したら `request_transition` を呼ぶ。詰まったら `stuck`。要件不足なら `back`。\n");
    Message {
        role: "user".to_string(),
        content: vec![ContentBlock::Text { text, cache_control: None }],
    }
}

#[cfg(test)]
pub(crate) fn test_initial_user_message_text(ctx: &WorkerContext) -> String {
    let m = initial_user_message(ctx);
    match &m.content[0] {
        ContentBlock::Text { text, .. } => text.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> WorkerContext {
        WorkerContext {
            system_prompt: "you are worker".into(),
            node_header: "n1 (implement)".into(),
            skill_body: "skill body".into(),
            spec_slice: "F-001 do X".into(),
            compact_status: "node 1/2 n1".into(),
            failed_gates: vec![],
            tools: vec!["read".into(), "edit".into()],
        }
    }

    #[test]
    fn system_blocks_carry_cache_control() {
        let bs = build_system_blocks(&ctx());
        assert_eq!(bs.len(), 2);
        for b in &bs {
            match b {
                ContentBlock::Text { cache_control, .. } => assert!(cache_control.is_some()),
                _ => panic!("not text"),
            }
        }
    }

    #[test]
    fn initial_user_includes_status_and_tools() {
        let text = test_initial_user_message_text(&ctx());
        assert!(text.contains("n1 (implement)"));
        assert!(text.contains("node 1/2 n1"));
        assert!(text.contains("read, edit"));
    }
}
