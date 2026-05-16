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
//!
//! 多言語版は `refs_lang::find_refs_for_lang` / `find_callers_for_lang` を使う。
//! 当ファイルの `find_refs` / `find_callers` は rust-analyzer 固定の薄ラッパとして残す
//! （backward compat）。`resolve_position` / `request_with_retry` /
//! `is_content_modified` は pub(super) で refs_lang から再利用する。

use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use super::client::LspClient;
use super::lang::Lang;
use super::refs_lang::{find_callers_for_lang, find_refs_for_lang};
use super::refs_parse::pick_best_match;

/// `workspace/symbol` empty 応答時のリトライ上限。indexing 中の一時空のみ吸収する。
/// no-hit qname で timeout (60s) を食い潰す旧挙動を防ぐ (layer 2.5 bench で発覚)。
const EMPTY_RETRY_ATTEMPTS: usize = 3;

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

/// `harness refs <qname>` ── rust-analyzer 固定の薄ラッパ。
///
/// `server_cmd` は API 互換のため受け取るが現在は使わず、`Lang::Rust` で固定する。
pub fn find_refs(
    _server_cmd: &str,
    root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<RefLocation>, String> {
    find_refs_for_lang(Lang::Rust, root, qname, timeout)
}

/// `harness callers <qname>` ── rust-analyzer 固定の薄ラッパ。
pub fn find_callers(
    _server_cmd: &str,
    root: &Path,
    qname: &str,
    timeout: Duration,
) -> Result<Vec<CallerInfo>, String> {
    find_callers_for_lang(Lang::Rust, root, qname, timeout)
}

/// `qname` を workspace/symbol で解決する（empty/content modified を short retry で吸収）。
///
/// 旧実装は timeout (60s) に達するまで empty を retry し続けていたため、
/// no-hit qname で 60s 張り付く問題があった (layer 2.5 bench 発覚)。
/// 現在は indexing 中の一時空のみ `EMPTY_RETRY_ATTEMPTS` 回まで許容する。
pub(super) fn resolve_position(
    client: &mut LspClient,
    qname: &str,
    timeout: Duration,
) -> Result<ResolvedPos, String> {
    let started = Instant::now();
    for attempt in 0..EMPTY_RETRY_ATTEMPTS {
        let resp = match client
            .request::<Value>("workspace/symbol", serde_json::json!({ "query": qname }))
        {
            Ok(v) => v,
            Err(e) if is_content_modified(&e) => Value::Array(Vec::new()),
            Err(e) => return Err(e),
        };
        if let Some(p) = pick_best_match(&resp, qname) {
            return Ok(p);
        }
        if attempt + 1 == EMPTY_RETRY_ATTEMPTS {
            break;
        }
        if started.elapsed() >= timeout {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(format!("symbol not found: {qname}"))
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
