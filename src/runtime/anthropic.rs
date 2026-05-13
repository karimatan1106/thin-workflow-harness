//! Anthropic Messages API（v2023-06-01）の最小限の型 ── tool-use ループ ＋ prompt caching 用。
//!
//! 仕様要点（`docs/implementation.md`「Anthropic API は生 HTTP で直叩き」、`DESIGN.md` §10 の "工夫 7"）:
//! - エンドポイント: `POST https://api.anthropic.com/v1/messages`
//! - headers: `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`
//! - `ContentBlock` に `cache_control = { type = "ephemeral" }` を付けると 5 分 TTL の prefix cache
//!   （system + skill + spec スライスに付けて、可変サフィックス＝status/feedback には付けない）
//! - tool-use: assistant が `ToolUse` ブロックを返す → 次の user メッセージで `ToolResult` を返す
//!
//! `tag` 名を `type` にして JSON 形のまま serialize/deserialize する（Anthropic の discriminant フィールド名）。

use serde::{Deserialize, Serialize};

/// prompt cache のヒント（5 分 TTL ephemeral）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheControl {
    /// 現状 `"ephemeral"` のみ。
    #[serde(rename = "type")]
    pub kind: String,
}

impl CacheControl {
    /// `{ "type": "ephemeral" }` を返す。
    pub fn ephemeral() -> Self {
        CacheControl { kind: "ephemeral".to_string() }
    }
}

/// メッセージ 1 個のコンテンツブロック（assistant の出力、user の入力どちらも同じ enum）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// テキスト本体。
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        cache_control: Option<CacheControl>,
    },
    /// assistant がツールを呼んでいる。
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// user 側がツール結果を返している。
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        is_error: Option<bool>,
    },
}

/// 1 メッセージ（role ＋ content blocks）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// `"user"` か `"assistant"`。
    pub role: String,
    pub content: Vec<ContentBlock>,
}

/// `tool_choice` 指定（既定は自動）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    /// 既定 ── モデルが判断する。
    Auto,
    /// 何か必ずツールを呼ぶ。
    Any,
    /// 指定ツール強制。
    Tool { name: String },
}

/// 渡すツール定義 1 個。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// JSON Schema（`type`, `properties`, `required`）。
    pub input_schema: serde_json::Value,
}

/// `POST /v1/messages` リクエスト本体。
#[derive(Debug, Clone, Serialize)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: u32,
    /// system は `Vec<ContentBlock>`（`cache_control` を載せたいので block 形式で送る）。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub system: Vec<ContentBlock>,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

/// 使用トークン数（cache hit/miss の内訳付き）。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

/// `POST /v1/messages` レスポンス（必要最小限）。
#[derive(Debug, Clone, Deserialize)]
pub struct MessagesResponse {
    pub content: Vec<ContentBlock>,
    /// `"end_turn"` / `"tool_use"` / `"max_tokens"` / `"stop_sequence"`。
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub usage: Usage,
}

impl MessagesResponse {
    /// ツール呼び出しブロックだけを抜く（順序保持）。
    pub fn tool_uses(&self) -> Vec<(&str, &str, &serde_json::Value)> {
        self.content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolUse { id, name, input } => Some((id.as_str(), name.as_str(), input)),
                _ => None,
            })
            .collect()
    }
}

/// 生 API のコスト目安テーブル（USD / 100 万トークン、`docs/operations.md` §1）。
/// **概算値**で正確な料金表ではない ── tokens が無い場合は `None`（メトリクスに cost を載せない）。
pub fn estimate_cost_usd(model: &str, u: &Usage) -> Option<f64> {
    let (input_per_m, output_per_m) = match model {
        m if m.starts_with("claude-opus") => (15.0_f64, 75.0_f64),
        m if m.starts_with("claude-sonnet") => (3.0_f64, 15.0_f64),
        m if m.starts_with("claude-haiku") => (1.0_f64, 5.0_f64),
        _ => return None,
    };
    let inp = u.input_tokens as f64 + u.cache_creation_input_tokens as f64
        + u.cache_read_input_tokens as f64 * 0.1; // cache read は 10% で概算
    let out = u.output_tokens as f64;
    Some(inp * input_per_m / 1_000_000.0 + out * output_per_m / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_control_serializes_with_type_field() {
        let cc = CacheControl::ephemeral();
        let j = serde_json::to_string(&cc).unwrap();
        assert_eq!(j, r#"{"type":"ephemeral"}"#);
    }

    #[test]
    fn content_block_text_with_cache_control() {
        let b = ContentBlock::Text {
            text: "hello".into(),
            cache_control: Some(CacheControl::ephemeral()),
        };
        let j = serde_json::to_string(&b).unwrap();
        assert!(j.contains(r#""type":"text""#));
        assert!(j.contains(r#""cache_control":{"type":"ephemeral"}"#));
    }

    #[test]
    fn tool_uses_extracts_only_tool_use_blocks() {
        let resp = MessagesResponse {
            content: vec![
                ContentBlock::Text { text: "think".into(), cache_control: None },
                ContentBlock::ToolUse {
                    id: "u1".into(),
                    name: "t".into(),
                    input: serde_json::json!({"a":1}),
                },
            ],
            stop_reason: Some("tool_use".into()),
            usage: Usage::default(),
        };
        assert_eq!(resp.tool_uses().len(), 1);
        assert_eq!(resp.tool_uses()[0].1, "t");
    }

    #[test]
    fn cost_estimate_known_models() {
        let u = Usage { input_tokens: 1_000_000, output_tokens: 1_000_000, ..Default::default() };
        let opus = estimate_cost_usd("claude-opus-4-7", &u).unwrap();
        assert!((opus - 90.0).abs() < 0.5);
        assert!(estimate_cost_usd("unknown-model-xyz", &u).is_none());
    }
}
