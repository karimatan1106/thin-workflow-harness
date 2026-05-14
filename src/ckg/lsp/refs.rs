//! `harness refs` / `harness callers` の LSP 層。
//!
//! - `find_refs(qname, root)`: workspace/symbol で qname の位置を解決 →
//!   textDocument/references で参照箇所一覧を返す。
//! - `find_callers(qname, root)`: 同じく位置解決 →
//!   callHierarchy/prepareCallHierarchy → callHierarchy/incomingCalls で
//!   呼び出し元（caller）一覧を返す。
//!
//! parser は `refs_parse` モジュールに切り出し。indexing は short retry で待つ
//! （rust-analyzer の `content modified` -32801 もリトライ対象）。

use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use super::client::LspClient;
use super::query::path_to_file_uri;
use super::refs_parse::{parse_incoming_calls, parse_references, pick_best_match};

/// `find_refs` が返す 1 件分。
#[derive(Debug, Clone, Serialize)]
pub struct RefLocation {
    pub file: String,
    pub line: usize,
    pub col: usize,
}

/// `find_callers` が返す 1 件分（incoming caller）。
#[derive(Debug, Clone, Serialize)]
pub struct CallerInfo {
    pub name: String,
    pub file: String,
    pub line: usize,
}

/// 位置解決の中間結果（URI + 0-origin line/character）。
pub(super) struct ResolvedPos {
    pub(super) uri: String,
    pub(super) line: u64,
    pub(super) character: u64,
}

impl ResolvedPos {
    pub(super) fn new(uri: String, line: u64, character: u64) -> Self {
        Self { uri, line, character }
    }
}

/// `harness refs <qname>` 本体。
pub fn find_refs(
    server_cmd: &str,
    root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<RefLocation>, String> {
    let mut client = LspClient::spawn(server_cmd)?;
    let root_uri = path_to_file_uri(root)?;
    let _ = client.initialize(&root_uri)?;

    let started = Instant::now();
    let pos = resolve_position(&mut client, qname, timeout)?;
    let params = json!({
        "textDocument": { "uri": pos.uri },
        "position": { "line": pos.line, "character": pos.character },
        "context": { "includeDeclaration": false },
    });
    let resp = request_with_retry(&mut client, "textDocument/references", params, timeout, started)?;
    let refs = parse_references(&resp);
    let _ = client.shutdown();
    Ok(refs)
}

/// `harness callers <qname>` 本体。
pub fn find_callers(
    server_cmd: &str,
    root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<CallerInfo>, String> {
    let mut client = LspClient::spawn(server_cmd)?;
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

/// `qname` を workspace/symbol で解決する（空 or content modified ならリトライ）。
pub(super) fn resolve_position(
    client: &mut LspClient,
    qname: &str,
    timeout: Duration,
) -> Result<ResolvedPos, String> {
    let started = Instant::now();
    loop {
        let resp = match client.request::<Value>("workspace/symbol", json!({ "query": qname })) {
            Ok(v) => v,
            Err(e) if is_content_modified(&e) => Value::Array(Vec::new()),
            Err(e) => return Err(e),
        };
        if let Some(p) = pick_best_match(&resp, qname) {
            return Ok(p);
        }
        if started.elapsed() >= timeout {
            return Err(format!("symbol not found: {qname}"));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// rust-analyzer の `content modified` (-32801) を short retry で吸収する request。
pub(super) fn request_with_retry(
    client: &mut LspClient,
    method: &str,
    params: Value,
    timeout: Duration,
    started: Instant,
) -> Result<Value, String> {
    loop {
        match client.request::<Value>(method, params.clone()) {
            Ok(v) => return Ok(v),
            Err(e) if is_content_modified(&e) => {
                if started.elapsed() >= timeout {
                    return Err(format!("{method}: timeout (last: {e})"));
                }
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}

pub(super) fn is_content_modified(e: &str) -> bool {
    e.contains("-32801") || e.contains("content modified")
}
