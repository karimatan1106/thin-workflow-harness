//! daemon wire format -- line-based JSON.
//!
//! Request / Response are each 1 line LF-terminated JSON object over TCP.
//! op uses serde tag/content so future ops (refs/callers) stay back-compat.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One request sent to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    #[serde(flatten)]
    pub op: Op,
}

/// One response returned by the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub data: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Supported ops. PoC has only find_symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", content = "params", rename_all = "snake_case")]
pub enum Op {
    FindSymbol(FindSymbolParams),
}

/// Params for find_symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindSymbolParams {
    pub qname: String,
    pub root: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    30_000
}

/// One symbol payload returned by the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolPayload {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
    pub col: usize,
}

impl From<crate::ckg::lsp::SymbolInfo> for SymbolPayload {
    fn from(s: crate::ckg::lsp::SymbolInfo) -> Self {
        SymbolPayload {
            name: s.name,
            kind: s.kind,
            file: s.file,
            line: s.line,
            col: s.col,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_find_symbol_request() {
        let raw = r#"{"id":1,"op":"find_symbol","params":{"qname":"User","root":"/tmp","kind":null,"timeout_ms":30000}}"#;
        let req: Request = serde_json::from_str(raw).expect("parse");
        assert_eq!(req.id, 1);
        match &req.op {
            Op::FindSymbol(p) => {
                assert_eq!(p.qname, "User");
                assert_eq!(p.root, "/tmp");
                assert!(p.kind.is_none());
                assert_eq!(p.timeout_ms, 30_000);
            }
        }
    }

    #[test]
    fn serialize_response_ok() {
        let resp = Response {
            id: 7,
            ok: true,
            data: serde_json::json!([{"name":"User","kind":"struct","file":"a.rs","line":3,"col":10}]),
            error: None,
        };
        let s = serde_json::to_string(&resp).expect("ser");
        assert!(s.contains("\"id\":7"));
        assert!(s.contains("\"ok\":true"));
        assert!(s.contains("User"));
        assert!(!s.contains("error"));
    }

    #[test]
    fn roundtrip_error_response() {
        let resp = Response {
            id: 2,
            ok: false,
            data: serde_json::Value::Null,
            error: Some("not found".to_string()),
        };
        let s = serde_json::to_string(&resp).expect("ser");
        let back: Response = serde_json::from_str(&s).expect("de");
        assert_eq!(back.id, 2);
        assert!(!back.ok);
        assert_eq!(back.error.as_deref(), Some("not found"));
    }

    #[test]
    fn default_timeout_when_missing() {
        let raw = r#"{"id":3,"op":"find_symbol","params":{"qname":"X","root":"/a"}}"#;
        let req: Request = serde_json::from_str(raw).expect("parse");
        let Op::FindSymbol(p) = &req.op;
        assert_eq!(p.timeout_ms, 30_000);
        assert!(p.kind.is_none());
    }
}
