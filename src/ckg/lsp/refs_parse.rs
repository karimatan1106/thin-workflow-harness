//! `refs` / `callers` 用の LSP レスポンス parser ── refs.rs から切り出し。
//!
//! - `pick_best_match`: workspace/symbol の配列から qname に最も合うものを 1 件選ぶ
//! - `parse_references`: textDocument/references の Location[] → RefLocation[]
//! - `parse_incoming_calls`: callHierarchy/incomingCalls の配列 → CallerInfo[]

use serde_json::Value;

use super::query::uri_to_path_string;
use super::refs::{CallerInfo, RefLocation, ResolvedPos};

/// workspace/symbol レスポンスから qname に最も合致する 1 件の位置を返す。
/// 1) name == qname、2) name == leaf（`::` 後ろ）、3) contains(leaf)、4) 最初の要素。
pub(super) fn pick_best_match(v: &Value, qname: &str) -> Option<ResolvedPos> {
    let arr = v.as_array()?;
    if arr.is_empty() {
        return None;
    }
    let leaf = qname.rsplit("::").next().unwrap_or(qname);
    let mut best: Option<(usize, &Value)> = None;
    for item in arr {
        let name = item.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let score = if name == qname {
            3
        } else if name == leaf {
            2
        } else if name.contains(leaf) {
            1
        } else {
            0
        };
        match best {
            Some((bs, _)) if score <= bs => {}
            _ => best = Some((score, item)),
        }
    }
    let (_, item) = best?;
    extract_resolved_pos(item)
}

fn extract_resolved_pos(item: &Value) -> Option<ResolvedPos> {
    let loc = item.get("location")?;
    let uri = loc.get("uri")?.as_str()?.to_string();
    let start = loc.get("range")?.get("start")?;
    let line = start.get("line")?.as_u64()?;
    let character = start.get("character")?.as_u64()?;
    Some(ResolvedPos::new(uri, line, character))
}

/// `textDocument/references` のレスポンス（Location[]）をパース。
pub(super) fn parse_references(v: &Value) -> Vec<RefLocation> {
    let arr = match v.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let uri = item.get("uri").and_then(|x| x.as_str()).unwrap_or("");
        let file = uri_to_path_string(uri);
        let start = item
            .get("range")
            .and_then(|r| r.get("start"))
            .cloned()
            .unwrap_or(Value::Null);
        let line = start.get("line").and_then(|x| x.as_u64()).unwrap_or(0) as usize + 1;
        let col =
            start.get("character").and_then(|x| x.as_u64()).unwrap_or(0) as usize + 1;
        out.push(RefLocation { file, line, col });
    }
    out
}

/// `callHierarchy/incomingCalls` のレスポンス（CallHierarchyIncomingCall[]）をパース。
pub(super) fn parse_incoming_calls(v: &Value) -> Vec<CallerInfo> {
    let arr = match v.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    let mut out = Vec::with_capacity(arr.len());
    for entry in arr {
        let from = match entry.get("from") {
            Some(f) => f,
            None => continue,
        };
        let name = from.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let uri = from.get("uri").and_then(|x| x.as_str()).unwrap_or("");
        let file = uri_to_path_string(uri);
        let line = from
            .get("range")
            .and_then(|r| r.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as usize
            + 1;
        out.push(CallerInfo { name, file, line });
    }
    out
}
