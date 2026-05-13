//! Anthropic API 認証モードの抽象（`ApiKey` / `Bearer`）。
//!
//! - `ANTHROPIC_API_KEY` env があれば `ApiKey(key)` ── `x-api-key: <key>` ヘッダ。
//! - 無ければ Claude Code の OAuth トークンを探す ── `CLAUDE_CODE_OAUTH_TOKEN` env か
//!   `~/.claude/.credentials.json`（Windows なら `%USERPROFILE%\.claude\.credentials.json`）の
//!   `access_token` フィールド ── `Bearer(token)` ── `Authorization: Bearer <token>` ヘッダ。
//! - どちらも無ければ明示的なエラー。
//!
//! `anthropic-beta` の運用: prompt caching を効かせるため両経路（ApiKey/Bearer）で
//! `prompt-caching-2024-07-31` を送る。Bearer 経路ではさらに MAX subscription 互換の
//! `oauth-2025-04-20` を追加（カンマ区切りで複数値）。refresh_token / expires_at は
//! 今回未実装 ── access_token を直接使う。

use std::path::PathBuf;

use crate::runtime::auth_credentials::extract_token;

/// prompt caching の `anthropic-beta` トークン（両経路で必須 ── これが無いと system block の
/// `cache_control: ephemeral` が hint として認識されず cache_creation/cache_read が 0 のまま）。
pub const PROMPT_CACHING_BETA: &str = "prompt-caching-2024-07-31";

/// MAX プラン OAuth 経路で追加する beta トークン（Bearer モード固有）。
/// `prompt-caching-2024-07-31` とカンマ区切りで連結して 1 つの header 値にする。
pub const OAUTH_BETA_TOKEN: &str = "oauth-2025-04-20";

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
    /// `anthropic-beta` には常に `prompt-caching-2024-07-31` が入る（無いと cache hint 無視）。
    /// Bearer モードでは `oauth-2025-04-20` も追加。
    pub fn auth_headers(&self, api_version: &str) -> Vec<(String, String)> {
        let mut h: Vec<(String, String)> = vec![
            ("anthropic-version".to_string(), api_version.to_string()),
        ];
        match self {
            AuthMode::ApiKey(key) => {
                h.push(("x-api-key".to_string(), key.clone()));
                h.push(("anthropic-beta".to_string(), PROMPT_CACHING_BETA.to_string()));
            }
            AuthMode::Bearer(tok) => {
                h.push(("authorization".to_string(), format!("Bearer {tok}")));
                h.push((
                    "anthropic-beta".to_string(),
                    format!("{PROMPT_CACHING_BETA},{OAUTH_BETA_TOKEN}"),
                ));
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

/// 既定の home dir 解決 ── Windows なら `%USERPROFILE%`、それ以外は `$HOME`。
fn default_home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}
