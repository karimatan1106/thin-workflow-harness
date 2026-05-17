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

/// `health` op の response payload。
///
/// - `status`: "ready" 固定 (warm-up 完了後の foreground daemon は ready 扱い)
/// - `lang`: "rust" | "ts" | "py" | "go"
/// - `uptime_ms`: daemon 起動からの経過時間
/// - `queries_handled`: 累計 query 数 (health op は除外)
/// - `recent_avg_ms`: 直近 N=10 件の wall time 平均 (0 = 未実行)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthPayload {
    pub status: String,
    pub lang: String,
    pub uptime_ms: u64,
    pub queries_handled: u64,
    pub recent_avg_ms: u64,
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
