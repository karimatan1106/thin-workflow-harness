//! `runtime/auth.rs` の統合テスト（前回まで `src/runtime/auth.rs` 末尾にあった unit test を移設）。
//!
//! `auth_credentials::extract_token` も lib の pub API として直接使う。
//! env を触らないテストだけ ── env 系は `resolve_auth_with` に注入する純粋関数版を呼ぶ。

use std::env::VarError;

use thin_workflow_harness_core::runtime::auth::{resolve_auth_with, AuthMode};
use thin_workflow_harness_core::runtime::auth_credentials::extract_token;

fn env_with<'a>(
    pairs: &'a [(&'a str, &'a str)],
) -> impl Fn(&str) -> Result<String, VarError> + 'a {
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
    // prompt caching を効かせるための beta header が ApiKey 経路にも必要。
    assert!(
        h.iter().any(|(k, v)| k == "anthropic-beta" && v.contains("prompt-caching-2024-07-31")),
        "api-key 経路に prompt-caching-2024-07-31 が無い: {h:?}",
    );
}

#[test]
fn auth_headers_bearer_form_has_beta() {
    let h = AuthMode::Bearer("oat".into()).auth_headers("2023-06-01");
    assert!(h.iter().any(|(k, v)| k == "authorization" && v == "Bearer oat"));
    assert!(!h.iter().any(|(k, _)| k == "x-api-key"));
    // Bearer は prompt-caching と oauth の 2 トークン両方を含むカンマ区切り header。
    let beta = h.iter().find(|(k, _)| k == "anthropic-beta").map(|(_, v)| v.clone());
    let beta = beta.expect("bearer に anthropic-beta header が無い");
    assert!(beta.contains("prompt-caching-2024-07-31"), "prompt-caching token 不在: {beta}");
    assert!(beta.contains("oauth-2025-04-20"), "oauth token 不在: {beta}");
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
