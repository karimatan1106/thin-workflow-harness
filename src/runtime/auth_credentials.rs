//! `~/.claude/.credentials.json` から access_token を抽出する純粋関数。
//!
//! 形式は非公開のため、「flat な `{access_token: ...}`」と「ネストされた
//! `{claudeAiOauth: {accessToken: ...}}` 系」両方を試す。

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FlatToken {
    #[serde(default, alias = "accessToken")]
    access_token: String,
}

/// JSON テキストから access_token を抜く ── 「flat な `{access_token: ...}`」と「ネストされた
/// `{claudeAiOauth: {accessToken: ...}}` 系」両方を試す（クレデンシャル仕様は非公開のため）。
pub fn extract_token(text: &str) -> Option<String> {
    // 1. flat: `{ "access_token": "...", ... }`
    if let Ok(flat) = serde_json::from_str::<FlatToken>(text) {
        if !flat.access_token.is_empty() {
            return Some(flat.access_token);
        }
    }
    // 2. nested: `{ "claudeAiOauth": { "accessToken": "...", ... } }` 等を Value で総当たり。
    let val: serde_json::Value = serde_json::from_str(text).ok()?;
    find_token_in_value(&val)
}

/// `Value` を再帰的に走査し、`access_token` or `accessToken` キーの文字列値を返す。
fn find_token_in_value(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Object(map) => {
            for key in ["access_token", "accessToken"] {
                if let Some(s) = map.get(key).and_then(|x| x.as_str()) {
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
            for (_, child) in map {
                if let Some(t) = find_token_in_value(child) {
                    return Some(t);
                }
            }
            None
        }
        _ => None,
    }
}
