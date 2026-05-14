//! `harness closure <qname>` の LSP 層 ── refs/callers の transitive 合成。
//!
//! - `direction=in`: callers の callers を depth まで BFS 再帰
//!   （`callHierarchy/incomingCalls.from` を次の seed として使う）
//! - `direction=out`: qname → references を 1 段だけ列挙（MVP、現状 depth=1 相当）
//! - `direction=both`: in と out を順に実行
//!
//! visited set + depth 制限で指数爆発を防ぐ。`request_with_retry` 経由で
//! content modified を吸収する。LspClient は 1 セッションだけ spawn し、
//! その中で BFS をぐるぐる回す。共通 helper は refs.rs を再利用。

use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use super::client::LspClient;
use super::query::{path_to_file_uri, uri_to_path_string};
use super::refs::{request_with_retry, resolve_position};
use super::refs_parse::{parse_incoming_calls, parse_references};

/// 方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction { In, Out, Both }

impl Direction {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "in" => Ok(Direction::In),
            "out" => Ok(Direction::Out),
            "both" => Ok(Direction::Both),
            other => Err(format!("unknown direction: {other} (in|out|both)")),
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self { Direction::In => "in", Direction::Out => "out", Direction::Both => "both" }
    }
}

/// closure 結果の 1 ノード。
#[derive(Debug, Clone, Serialize)]
pub struct ClosureNode {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub depth: usize,
    pub direction: String,
}

/// 上限 depth。指数爆発防止。
pub const MAX_DEPTH: usize = 5;

/// `harness closure <qname>` 本体。
pub fn find_closure(
    server_cmd: &str,
    root: &Path,
    qname: &str,
    depth: usize,
    direction: Direction,
    timeout: Duration,
) -> Result<Vec<ClosureNode>, String> {
    let depth = depth.clamp(1, MAX_DEPTH);
    let mut client = LspClient::spawn(server_cmd)?;
    let root_uri = path_to_file_uri(root)?;
    let _ = client.initialize(&root_uri)?;
    let started = Instant::now();

    let mut nodes: Vec<ClosureNode> = Vec::new();
    if matches!(direction, Direction::In | Direction::Both) {
        nodes.extend(closure_in(&mut client, qname, depth, timeout, started)?);
    }
    if matches!(direction, Direction::Out | Direction::Both) {
        nodes.extend(closure_out(&mut client, qname, timeout, started)?);
    }
    let _ = client.shutdown();
    Ok(nodes)
}

/// `direction=in`: callHierarchy/incomingCalls の transitive BFS。
fn closure_in(
    client: &mut LspClient,
    qname: &str,
    depth: usize,
    timeout: Duration,
    started: Instant,
) -> Result<Vec<ClosureNode>, String> {
    let pos = resolve_position(client, qname, timeout)?;
    let prepare = request_with_retry(
        client,
        "textDocument/prepareCallHierarchy",
        json!({
            "textDocument": { "uri": pos.uri },
            "position": { "line": pos.line, "character": pos.character },
        }),
        timeout, started,
    )?;
    let seed_items = match prepare.as_array() {
        Some(a) if !a.is_empty() => a.clone(),
        _ => return Ok(Vec::new()),
    };
    let mut visited: HashSet<String> = HashSet::new();
    let mut out: Vec<ClosureNode> = Vec::new();
    let mut queue: VecDeque<(Value, usize)> = VecDeque::new();
    for it in &seed_items {
        visited.insert(item_key(it));
        queue.push_back((it.clone(), 0));
    }
    while let Some((item, d)) = queue.pop_front() {
        if d >= depth { continue; }
        let resp = request_with_retry(
            client, "callHierarchy/incomingCalls",
            json!({ "item": item }), timeout, started,
        )?;
        let callers = parse_incoming_calls(&resp);
        let arr = resp.as_array().cloned().unwrap_or_default();
        for (caller, raw) in callers.iter().zip(arr.iter()) {
            let from = match raw.get("from") { Some(f) => f, None => continue };
            let key = item_key(from);
            if visited.contains(&key) { continue; }
            visited.insert(key);
            out.push(ClosureNode {
                name: caller.name.clone(),
                file: caller.file.clone(),
                line: caller.line,
                depth: d + 1,
                direction: "in".to_string(),
            });
            queue.push_back((from.clone(), d + 1));
        }
    }
    Ok(out)
}

/// `direction=out`: refs の transitive（MVP、depth=1 相当の 1 段）。
fn closure_out(
    client: &mut LspClient,
    qname: &str,
    timeout: Duration,
    started: Instant,
) -> Result<Vec<ClosureNode>, String> {
    let pos = resolve_position(client, qname, timeout)?;
    let resp = request_with_retry(
        client, "textDocument/references",
        json!({
            "textDocument": { "uri": pos.uri },
            "position": { "line": pos.line, "character": pos.character },
            "context": { "includeDeclaration": false },
        }),
        timeout, started,
    )?;
    let refs = parse_references(&resp);
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(refs.len());
    for r in refs {
        let key = format!("{}:{}", r.file, r.line);
        if !seen.insert(key) { continue; }
        out.push(ClosureNode {
            name: String::new(),
            file: r.file, line: r.line,
            depth: 1, direction: "out".to_string(),
        });
    }
    Ok(out)
}

/// CallHierarchyItem 由来の visited key（uri + line + character）。
fn item_key(item: &Value) -> String {
    let uri = item.get("uri").and_then(|x| x.as_str()).unwrap_or("");
    let start = item.get("range").and_then(|r| r.get("start"));
    let line = start.and_then(|s| s.get("line")).and_then(|x| x.as_u64()).unwrap_or(0);
    let ch = start.and_then(|s| s.get("character")).and_then(|x| x.as_u64()).unwrap_or(0);
    format!("{}|{}:{}", uri_to_path_string(uri), line, ch)
}
