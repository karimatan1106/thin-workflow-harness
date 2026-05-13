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

use crate::runtime::auth_credentials::extract_token;

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

/// 既定の home dir 解決 ── Windows なら `%USERPROFILE%`、それ以外は `$HOME`。
fn default_home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}
