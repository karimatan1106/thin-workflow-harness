//! `protocol.rs` の serde 形式テスト ── 200 行制約のため protocol.rs から切り出し。

use super::protocol::*;
use serde_json::Value;

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
        other => panic!("unexpected op: {other:?}"),
    }
}

#[test]
fn parse_refs_request() {
    let raw = r#"{"id":2,"op":"refs","params":{"qname":"User","root":"/tmp"}}"#;
    let req: Request = serde_json::from_str(raw).expect("parse");
    match &req.op {
        Op::Refs(p) => {
            assert_eq!(p.qname, "User");
            assert_eq!(p.timeout_ms, 30_000);
        }
        other => panic!("unexpected op: {other:?}"),
    }
}

#[test]
fn parse_closure_request_with_defaults() {
    let raw = r#"{"id":3,"op":"closure","params":{"qname":"X","root":"/a"}}"#;
    let req: Request = serde_json::from_str(raw).expect("parse");
    match &req.op {
        Op::Closure(p) => {
            assert_eq!(p.depth, 2);
            assert_eq!(p.direction, "in");
            assert_eq!(p.timeout_ms, 30_000);
        }
        other => panic!("unexpected op: {other:?}"),
    }
}

#[test]
fn parse_tested_by_request_defaults_depth_to_3() {
    let raw = r#"{"id":4,"op":"tested_by","params":{"qname":"X","root":"/a"}}"#;
    let req: Request = serde_json::from_str(raw).expect("parse");
    match &req.op {
        Op::TestedBy(p) => {
            assert_eq!(p.depth, 3);
        }
        other => panic!("unexpected op: {other:?}"),
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
        data: Value::Null,
        error: Some("not found".to_string()),
    };
    let s = serde_json::to_string(&resp).expect("ser");
    let back: Response = serde_json::from_str(&s).expect("de");
    assert_eq!(back.id, 2);
    assert!(!back.ok);
    assert_eq!(back.error.as_deref(), Some("not found"));
}
