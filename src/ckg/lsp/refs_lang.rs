//! Lang 引数版の `find_refs` / `find_callers` / `find_outgoing`。
//! layer 2.5 PoC で `_with_client` 版を分離。既存 API は内部で `_with_client`
//! を呼ぶ薄いラッパに退避。

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::json;

use super::client::{start_and_warm_up, LspClient};
use super::lang::Lang;
use super::refs::{
    request_with_retry, resolve_position, CallerInfo, RefLocation,
};
use super::refs_parse::{parse_incoming_calls, parse_outgoing_calls, parse_references};

/// `find_refs` の Lang 版 (既存 fire-and-forget API)。
pub fn find_refs_for_lang(
    lang: Lang,
    root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<RefLocation>, String> {
    let mut client = start_and_warm_up(lang, root)?;
    let result = find_refs_for_lang_with_client(&mut client, lang, root, qname, timeout);
    let _ = client.shutdown();
    result
}

/// `find_refs_for_lang` の client 再利用版。
pub fn find_refs_for_lang_with_client(
    client: &mut LspClient,
    _lang: Lang,
    _root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<RefLocation>, String> {
    let started = Instant::now();
    let pos = resolve_position(client, qname, timeout)?;
    let params = json!({
        "textDocument": { "uri": pos.uri },
        "position": { "line": pos.line, "character": pos.character },
        "context": { "includeDeclaration": false },
    });
    let resp = request_with_retry(
        client,
        "textDocument/references",
        params,
        timeout,
        started,
    )?;
    Ok(parse_references(&resp))
}

/// `find_callers` の Lang 版 (既存 fire-and-forget API)。
pub fn find_callers_for_lang(
    lang: Lang,
    root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<CallerInfo>, String> {
    let mut client = start_and_warm_up(lang, root)?;
    let result = find_callers_for_lang_with_client(&mut client, lang, root, qname, timeout);
    let _ = client.shutdown();
    result
}

/// `find_callers_for_lang` の client 再利用版。
pub fn find_callers_for_lang_with_client(
    client: &mut LspClient,
    _lang: Lang,
    _root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<CallerInfo>, String> {
    let started = Instant::now();
    let pos = resolve_position(client, qname, timeout)?;
    let prepare_params = json!({
        "textDocument": { "uri": pos.uri },
        "position": { "line": pos.line, "character": pos.character },
    });
    let prepare = request_with_retry(
        client,
        "textDocument/prepareCallHierarchy",
        prepare_params,
        timeout,
        started,
    )?;
    let items = match prepare.as_array() {
        Some(a) if !a.is_empty() => a.clone(),
        _ => return Ok(Vec::new()),
    };

    let mut callers: Vec<CallerInfo> = Vec::new();
    for item in &items {
        let resp = request_with_retry(
            client,
            "callHierarchy/incomingCalls",
            json!({ "item": item }),
            timeout,
            started,
        )?;
        callers.extend(parse_incoming_calls(&resp));
    }
    Ok(callers)
}

/// `find_outgoing` の Lang 版 (既存 fire-and-forget API)。
pub fn find_outgoing_for_lang(
    lang: Lang,
    root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<CallerInfo>, String> {
    let mut client = start_and_warm_up(lang, root)?;
    let result = find_outgoing_for_lang_with_client(&mut client, lang, root, qname, timeout);
    let _ = client.shutdown();
    result
}

/// `find_outgoing_for_lang` の client 再利用版。
pub fn find_outgoing_for_lang_with_client(
    client: &mut LspClient,
    _lang: Lang,
    _root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<CallerInfo>, String> {
    let started = Instant::now();
    let pos = resolve_position(client, qname, timeout)?;
    let prepare_params = json!({
        "textDocument": { "uri": pos.uri },
        "position": { "line": pos.line, "character": pos.character },
    });
    let prepare = request_with_retry(
        client,
        "textDocument/prepareCallHierarchy",
        prepare_params,
        timeout,
        started,
    )?;
    let items = match prepare.as_array() {
        Some(a) if !a.is_empty() => a.clone(),
        _ => return Ok(Vec::new()),
    };

    let mut callees: Vec<CallerInfo> = Vec::new();
    for item in &items {
        let resp = request_with_retry(
            client,
            "callHierarchy/outgoingCalls",
            json!({ "item": item }),
            timeout,
            started,
        )?;
        callees.extend(parse_outgoing_calls(&resp));
    }
    Ok(callees)
}
