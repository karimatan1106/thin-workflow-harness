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

/// 渡すツール定義 1 個。`cache_control` を最後のツールに付けると
/// system + tools 全体が cache prefix になり、1024 input token 閾値を確実に超える。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// JSON Schema（`type`, `properties`, `required`）。
    pub input_schema: serde_json::Value,
    /// `Some(ephemeral)` で tools 配列の cache 区切りマーカーになる。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cache_control: Option<CacheControl>,
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

/// モデルのティアエイリアス（`opus`/`sonnet`/`haiku`/`fable`）を具体モデル ID に解決する。
/// 既に具体 ID（その他の文字列）はそのまま通す（後方互換 ── 既存 workflow.toml に
/// 直接書かれた `claude-...` もそのまま動く）。
///
/// **このプロジェクトで具体モデル ID を書く唯一の拠点。** Anthropic API は具体 ID を
/// 要求するため版番号はここに 1 箇所だけ集約し、テンプレ／プロンプト／docs には
/// エイリアスだけを書く。各ティアは `HARNESS_MODEL_<TIER>` 環境変数で上書きできる
/// （例 `HARNESS_MODEL_OPUS=claude-opus-4-9`）。
pub fn resolve_model(name: &str) -> String {
    resolve_model_with(name, |k| std::env::var(k).ok())
}

/// `resolve_model` の env 注入版（テスト用）。
pub fn resolve_model_with<F>(name: &str, env: F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    let n = name.trim();
    let (tier_env, default_id) = match n.to_ascii_lowercase().as_str() {
        "opus" => ("HARNESS_MODEL_OPUS", "claude-opus-4-8"),
        "sonnet" => ("HARNESS_MODEL_SONNET", "claude-sonnet-4-6"),
        "haiku" => ("HARNESS_MODEL_HAIKU", "claude-haiku-4-5-20251001"),
        "fable" => ("HARNESS_MODEL_FABLE", "claude-fable-5"),
        // エイリアスでなければ具体 ID とみなしてそのまま返す。
        _ => return n.to_string(),
    };
    env(tier_env)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn resolve_model_maps_tier_aliases_to_concrete_ids() {
        assert_eq!(resolve_model_with("opus", no_env), "claude-opus-4-8");
        assert_eq!(resolve_model_with("sonnet", no_env), "claude-sonnet-4-6");
        assert_eq!(resolve_model_with("haiku", no_env), "claude-haiku-4-5-20251001");
        // 大小無視・前後空白許容。
        assert_eq!(resolve_model_with("  OPUS ", no_env), "claude-opus-4-8");
    }

    #[test]
    fn resolve_model_passes_through_concrete_ids() {
        // 具体 ID はそのまま（後方互換）。
        assert_eq!(resolve_model_with("claude-opus-4-9", no_env), "claude-opus-4-9");
        assert_eq!(resolve_model_with("custom-model", no_env), "custom-model");
    }

    #[test]
    fn resolve_model_env_override_wins() {
        let env = |k: &str| (k == "HARNESS_MODEL_OPUS").then(|| "claude-opus-9-9".to_string());
        assert_eq!(resolve_model_with("opus", env), "claude-opus-9-9");
        // 空文字 override は無視して既定にフォールバック。
        let empty = |_: &str| Some("  ".to_string());
        assert_eq!(resolve_model_with("sonnet", empty), "claude-sonnet-4-6");
    }

    /// resolve した具体 ID が価格表のファミリ判定に乗ること（resolver→pricing の結線）。
    #[test]
    fn resolved_alias_is_priceable() {
        let u = Usage { input_tokens: 1_000_000, output_tokens: 1_000_000, ..Default::default() };
        let id = resolve_model_with("opus", no_env);
        assert!(estimate_cost_usd(&id, &u).is_some());
    }

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
