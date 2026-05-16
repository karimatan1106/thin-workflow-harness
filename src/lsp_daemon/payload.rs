//! daemon response payload types -- 1 struct per op data shape.
//!
//! Response.data carries one of these as JSON. Each `From<Domain>` conversion
//! mirrors the upstream `ckg::lsp` type so that server-side dispatch can
//! collect/serialize and client-side can deserialize the same shape.

use serde::{Deserialize, Serialize};

use crate::ckg::lsp::impacted::ImpactedNode;
use crate::ckg::lsp::{
    CallerInfo, ClosureNode, RefLocation, SymbolInfo, TestedNode,
};

/// One symbol returned by `find_symbol`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolPayload {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
    pub col: usize,
}

impl From<SymbolInfo> for SymbolPayload {
    fn from(s: SymbolInfo) -> Self {
        SymbolPayload {
            name: s.name,
            kind: s.kind,
            file: s.file,
            line: s.line,
            col: s.col,
        }
    }
}

/// One reference location returned by `refs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefPayload {
    pub file: String,
    pub line: usize,
    pub col: usize,
}

impl From<RefLocation> for RefPayload {
    fn from(r: RefLocation) -> Self {
        RefPayload { file: r.file, line: r.line, col: r.col }
    }
}

/// One caller / callee returned by `callers` / `outgoing`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallerPayload {
    pub name: String,
    pub file: String,
    pub line: usize,
}

impl From<CallerInfo> for CallerPayload {
    fn from(c: CallerInfo) -> Self {
        CallerPayload { name: c.name, file: c.file, line: c.line }
    }
}

/// One closure node returned by `closure`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosureNodePayload {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub depth: usize,
    pub direction: String,
}

impl From<ClosureNode> for ClosureNodePayload {
    fn from(n: ClosureNode) -> Self {
        ClosureNodePayload {
            name: n.name,
            file: n.file,
            line: n.line,
            depth: n.depth,
            direction: n.direction,
        }
    }
}

/// One node returned by `impacted_by` / `tested_by`. Direction is implicit
/// ("in") so we drop the field versus ClosureNodePayload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestedNodePayload {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub depth: usize,
}

impl From<TestedNode> for TestedNodePayload {
    fn from(n: TestedNode) -> Self {
        TestedNodePayload { name: n.name, file: n.file, line: n.line, depth: n.depth }
    }
}

impl From<ImpactedNode> for TestedNodePayload {
    fn from(n: ImpactedNode) -> Self {
        TestedNodePayload { name: n.name, file: n.file, line: n.line, depth: n.depth }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_payload_roundtrip() {
        let p = SymbolPayload {
            name: "User".to_string(),
            kind: "struct".to_string(),
            file: "a.rs".to_string(),
            line: 3,
            col: 10,
        };
        let s = serde_json::to_string(&p).expect("ser");
        let back: SymbolPayload = serde_json::from_str(&s).expect("de");
        assert_eq!(back.name, "User");
        assert_eq!(back.kind, "struct");
    }

    #[test]
    fn closure_node_payload_serializes_direction() {
        let p = ClosureNodePayload {
            name: "f".to_string(),
            file: "x.rs".to_string(),
            line: 1,
            depth: 2,
            direction: "in".to_string(),
        };
        let s = serde_json::to_string(&p).expect("ser");
        assert!(s.contains("\"direction\":\"in\""));
    }

    #[test]
    fn tested_node_payload_omits_direction() {
        let p = TestedNodePayload {
            name: "test_f".to_string(),
            file: "x.rs".to_string(),
            line: 9,
            depth: 1,
        };
        let s = serde_json::to_string(&p).expect("ser");
        assert!(!s.contains("direction"));
    }
}
