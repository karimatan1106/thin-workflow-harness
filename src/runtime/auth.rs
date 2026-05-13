//! Anthropic API 認証モードの抽象（`ApiKey` / `Bearer`）。
//!
//! - `ANTHROPIC_API_KEY` env があれば `ApiKey(key)` ── `x-api-key: <key>` ヘッダ。
//! - 無ければ Claude Code の OAuth トークンを探す ── `CLAUDE_CODE_OAUTH_TOKEN` env か
//!   `~/.claude/.credentials.json`（Windows なら `%USERPROFILE%\.claude\.credentials.json`）の
//!   `access_token` フィールド ── `Bearer(token)` ── `Authorization: Bearer <token>` ヘッダ。
//! - どちらも無ければ明示的なエラー。
//!
//! 要確認: `Bearer` 時の `anthropic-beta` ヘッダ（MAX subscription billing を有効にする値、
//! 実環境でテスト時に確定）。refresh_token / expires_at は今回未実装 ── access_token を直接使う。

use std::path::PathBuf;

use serde::Deserialize;

/// MAX プラン OAuth 経路で念のため付与する beta header の値。
/// **要確認**: 公開ドキュメントに正式値が無いため、実環境で適切な値を確定すること。
/// 現状は「Bearer 経路を使うシグナル」として暫定値を入れているだけで、Anthropic 側で
/// 認識されない場合は無視されるはず（不要なら削除）。
pub const OAUTH_BETA_HEADER: &str = "oauth-2025-04-20";

/// 認証戦略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMode {
    /// 通常 API キー（`x-api-key` ヘッダ）。
    ApiKey(String),
    /// OAuth Bearer（MAX プラン subscription、`Authorization: Bearer` ヘッダ）。
    Bearer(String),
}

impl AuthMode {
    /// HTTP リクエストの認証/バージョン関連ヘッダ列を返す。`content-type` は呼び出し側で付ける。
    pub fn auth_headers(&self, api_version: &str) -> Vec<(String, String)> {
        let mut h: Vec<(String, String)> = vec![
            ("anthropic-version".to_string(), api_version.to_string()),
        ];
        match self {
            AuthMode::ApiKey(key) => {
                h.push(("x-api-key".to_string(), key.clone()));
            }
            AuthMode::Bearer(tok) => {
                h.push(("authorization".to_string(), format!("Bearer {tok}")));
                // 要確認: 実環境で正しい beta header を確定。
                h.push(("anthropic-beta".to_string(), OAUTH_BETA_HEADER.to_string()));
            }
        }
        h
    }
}

/// Anthropic 認証情報を解決する。`env_lookup` と `home_dir` を渡せるのでテストで差し替え可能。
pub fn resolve_auth() -> Result<AuthMode, String> {
    resolve_auth_with(|k: &str| std::env::var(k), default_home_dir)
}

/// `resolve_auth` の純粋関数版（テスト注入用）。
pub fn resolve_auth_with<F, H>(env_lookup: F, home_dir: H) -> Result<AuthMode, String>
where
    F: Fn(&str) -> Result<String, std::env::VarError>,
    H: Fn() -> Option<PathBuf>,
{
    if let Ok(key) = env_lookup("ANTHROPIC_API_KEY") {
        if !key.trim().is_empty() {
            return Ok(AuthMode::ApiKey(key));
        }
    }
    if let Ok(tok) = env_lookup("CLAUDE_CODE_OAUTH_TOKEN") {
        if !tok.trim().is_empty() {
            return Ok(AuthMode::Bearer(tok));
        }
    }
    if let Some(home) = home_dir() {
        let path = home.join(".claude").join(".credentials.json");
        if let Some(tok) = try_read_oauth_credentials(&path)? {
            return Ok(AuthMode::Bearer(tok));
        }
    }
    Err(
        "ANTHROPIC_API_KEY を設定するか、Claude Code にログイン \
         （`~/.claude/.credentials.json` を作成）するか、`--script` を渡してください"
            .to_string(),
    )
}

/// `~/.claude/.credentials.json` を最小限の serde で試す ── 形式不明なので
/// 「`access_token` が文字列で取れれば OK、それ以外は None」。
fn try_read_oauth_credentials(path: &std::path::Path) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("OAuth credentials 読取失敗 {}: {e}", path.display()))?;
    if let Some(tok) = extract_token(&text) {
        return Ok(Some(tok));
    }
    Ok(None)
}

/// JSON テキストから access_token を抜く ── 「flat な `{access_token: ...}`」と「ネストされた
/// `{claudeAiOauth: {accessToken: ...}}` 系」両方を試す（クレデンシャル仕様は非公開のため）。
fn extract_token(text: &str) -> Option<String> {
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

#[derive(Debug, Deserialize)]
struct FlatToken {
    #[serde(default, alias = "accessToken")]
    access_token: String,
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

/// 既定の home dir 解決 ── Windows なら `%USERPROFILE%`、それ以外は `$HOME`。
fn default_home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::VarError;

    fn env_with<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Result<String, VarError> + 'a {
        move |k: &str| {
            for (pk, pv) in pairs {
                if *pk == k {
                    return Ok((*pv).to_string());
                }
            }
            Err(VarError::NotPresent)
        }
    }

    #[test]
    fn api_key_env_takes_priority() {
        let r = resolve_auth_with(env_with(&[("ANTHROPIC_API_KEY", "sk-real")]), || None).unwrap();
        assert_eq!(r, AuthMode::ApiKey("sk-real".into()));
    }

    #[test]
    fn bearer_env_used_when_api_key_missing() {
        let r = resolve_auth_with(env_with(&[("CLAUDE_CODE_OAUTH_TOKEN", "oat-x")]), || None).unwrap();
        assert_eq!(r, AuthMode::Bearer("oat-x".into()));
    }

    #[test]
    fn both_missing_is_error() {
        let r = resolve_auth_with(env_with(&[]), || None);
        assert!(r.is_err());
        let msg = r.unwrap_err();
        assert!(msg.contains("ANTHROPIC_API_KEY"));
        assert!(msg.contains("Claude Code"));
        assert!(msg.contains("--script"));
    }

    #[test]
    fn empty_env_value_falls_through() {
        let r = resolve_auth_with(env_with(&[("ANTHROPIC_API_KEY", "")]), || None);
        assert!(r.is_err(), "empty key should not match");
    }

    #[test]
    fn credentials_json_flat_form_parsed() {
        let json = r#"{"access_token":"oat-flat","refresh_token":"r"}"#;
        assert_eq!(extract_token(json).as_deref(), Some("oat-flat"));
    }

    #[test]
    fn credentials_json_nested_form_parsed() {
        let json = r#"{"claudeAiOauth":{"accessToken":"oat-nested","expiresAt":123}}"#;
        assert_eq!(extract_token(json).as_deref(), Some("oat-nested"));
    }

    #[test]
    fn credentials_json_missing_token_is_none() {
        let json = r#"{"refreshToken":"r-only"}"#;
        assert!(extract_token(json).is_none());
    }

    #[test]
    fn auth_headers_api_key_form() {
        let h = AuthMode::ApiKey("sk-x".into()).auth_headers("2023-06-01");
        assert!(h.iter().any(|(k, v)| k == "x-api-key" && v == "sk-x"));
        assert!(h.iter().any(|(k, v)| k == "anthropic-version" && v == "2023-06-01"));
        assert!(!h.iter().any(|(k, _)| k == "authorization"));
    }

    #[test]
    fn auth_headers_bearer_form_has_beta() {
        let h = AuthMode::Bearer("oat".into()).auth_headers("2023-06-01");
        assert!(h.iter().any(|(k, v)| k == "authorization" && v == "Bearer oat"));
        assert!(h.iter().any(|(k, _)| k == "anthropic-beta"));
        assert!(!h.iter().any(|(k, _)| k == "x-api-key"));
    }

    #[test]
    fn credentials_file_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let cred = dir.path().join(".claude").join(".credentials.json");
        std::fs::create_dir_all(cred.parent().unwrap()).unwrap();
        std::fs::write(&cred, r#"{"access_token":"oat-disk"}"#).unwrap();
        let home = dir.path().to_path_buf();
        let r = resolve_auth_with(env_with(&[]), || Some(home.clone())).unwrap();
        assert_eq!(r, AuthMode::Bearer("oat-disk".into()));
    }
}
