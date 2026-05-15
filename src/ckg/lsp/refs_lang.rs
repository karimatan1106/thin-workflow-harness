//! Lang 引数版の `find_refs` / `find_callers`。既存 `refs::*` の多言語ラッパで、
//! `Lang` から server コマンドを解決して spawn する。
//!
//! 200 行制約のため refs.rs から分離。
//! LSP request は `workspace/symbol` → `textDocument/references` /
//! `callHierarchy/prepareCallHierarchy` → `callHierarchy/incomingCalls`。
//! 各 server (rust-analyzer / typescript-language-server) が文法差を吸収する。

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::json;

use super::client::LspClient;
use super::lang::Lang;
use super::query::path_to_file_uri;
use super::refs::{
    request_with_retry, resolve_position, CallerInfo, RefLocation,
};
use super::refs_parse::{parse_incoming_calls, parse_outgoing_calls, parse_references};

/// `find_refs` の Lang 版。
///
/// - Rust → rust-analyzer
/// - Ts   → typescript-language-server --stdio
pub fn find_refs_for_lang(
    lang: Lang,
    root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<RefLocation>, String> {
    let mut client = LspClient::start_for_lang(lang)?;
    let root_uri = path_to_file_uri(root)?;
    let _ = client.initialize(&root_uri)?;

    let started = Instant::now();
    let pos = resolve_position(&mut client, qname, timeout)?;
    let params = json!({
        "textDocument": { "uri": pos.uri },
        "position": { "line": pos.line, "character": pos.character },
        "context": { "includeDeclaration": false },
    });
    let resp = request_with_retry(
        &mut client,
        "textDocument/references",
        params,
        timeout,
        started,
    )?;
    let refs = parse_references(&resp);
    let _ = client.shutdown();
    Ok(refs)
}

/// `find_callers` の Lang 版。callHierarchy/prepareCallHierarchy →
/// callHierarchy/incomingCalls の 2 段。
pub fn find_callers_for_lang(
    lang: Lang,
    root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<CallerInfo>, String> {
    let mut client = LspClient::start_for_lang(lang)?;
    let root_uri = path_to_file_uri(root)?;
    let _ = client.initialize(&root_uri)?;

    let started = Instant::now();
    let pos = resolve_position(&mut client, qname, timeout)?;
    let prepare_params = json!({
        "textDocument": { "uri": pos.uri },
        "position": { "line": pos.line, "character": pos.character },
    });
    let prepare = request_with_retry(
        &mut client,
        "textDocument/prepareCallHierarchy",
        prepare_params,
        timeout,
        started,
    )?;
    let items = match prepare.as_array() {
        Some(a) if !a.is_empty() => a.clone(),
        _ => {
            let _ = client.shutdown();
            return Ok(Vec::new());
        }
    };

    let mut callers: Vec<CallerInfo> = Vec::new();
    for item in &items {
        let resp = request_with_retry(
            &mut client,
            "callHierarchy/incomingCalls",
            json!({ "item": item }),
            timeout,
            started,
        )?;
        callers.extend(parse_incoming_calls(&resp));
    }
    let _ = client.shutdown();
    Ok(callers)
}

/// `find_callers_for_lang` の outgoing 版。`prepareCallHierarchy` →
/// `callHierarchy/outgoingCalls` の 2 段で、qname が呼び出している関数一覧（1 段）を返す。
/// 戻り値の `CallerInfo` は構造的に同じだが、意味は「callee（呼び出し先）」。
pub fn find_outgoing_for_lang(
    lang: Lang,
    root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<CallerInfo>, String> {
    let mut client = LspClient::start_for_lang(lang)?;
    let root_uri = path_to_file_uri(root)?;
    let _ = client.initialize(&root_uri)?;

    let started = Instant::now();
    let pos = resolve_position(&mut client, qname, timeout)?;
    let prepare_params = json!({
        "textDocument": { "uri": pos.uri },
        "position": { "line": pos.line, "character": pos.character },
    });
    let prepare = request_with_retry(
        &mut client,
        "textDocument/prepareCallHierarchy",
        prepare_params,
        timeout,
        started,
    )?;
    let items = match prepare.as_array() {
        Some(a) if !a.is_empty() => a.clone(),
        _ => {
            let _ = client.shutdown();
            return Ok(Vec::new());
        }
    };

    let mut callees: Vec<CallerInfo> = Vec::new();
    for item in &items {
        let resp = request_with_retry(
            &mut client,
            "callHierarchy/outgoingCalls",
            json!({ "item": item }),
            timeout,
            started,
        )?;
        callees.extend(parse_outgoing_calls(&resp));
    }
    let _ = client.shutdown();
    Ok(callees)
}
