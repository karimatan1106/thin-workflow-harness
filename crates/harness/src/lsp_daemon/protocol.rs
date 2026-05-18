//! daemon wire format -- line-based JSON.
//!
//! Request / Response are each 1 line LF-terminated JSON object over TCP.
//! `op` uses serde tag/content so future ops stay back-compat.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One request sent to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    #[serde(flatten)]
    pub op: Op,
}

/// One response returned by the daemon. `data` is a polymorphic JSON value
/// (e.g. `Vec<SymbolPayload>` / `Vec<RefPayload>` / ...). Clients deserialize
/// based on the op they sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub data: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// All supported daemon ops.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", content = "params", rename_all = "snake_case")]
pub enum Op {
    FindSymbol(FindSymbolParams),
    Refs(RefsParams),
    Callers(CallersParams),
    Outgoing(OutgoingParams),
    Closure(ClosureParams),
    ImpactedBy(ImpactedByParams),
    TestedBy(TestedByParams),
    Health(HealthParams),
}

/// Params for `health`. 現状 empty struct (将来拡張用に予約)。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthParams {}

/// Params for `find_symbol`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindSymbolParams {
    pub qname: String,
    pub root: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

/// Params for `refs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefsParams {
    pub qname: String,
    pub root: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

/// Params for `callers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallersParams {
    pub qname: String,
    pub root: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

/// Params for `outgoing` (closure direction=out primitive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingParams {
    pub qname: String,
    pub root: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

/// Params for `closure`. `direction` is "in" | "out" | "both".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosureParams {
    pub qname: String,
    #[serde(default = "default_closure_depth")]
    pub depth: usize,
    #[serde(default = "default_direction")]
    pub direction: String,
    pub root: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

/// Params for `impacted_by` (closure direction=in wrapper).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactedByParams {
    pub qname: String,
    #[serde(default = "default_impacted_depth")]
    pub depth: usize,
    pub root: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

/// Params for `tested_by` (closure direction=in + test filter).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestedByParams {
    pub qname: String,
    #[serde(default = "default_impacted_depth")]
    pub depth: usize,
    pub root: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    30_000
}

fn default_closure_depth() -> usize {
    2
}

fn default_impacted_depth() -> usize {
    3
}

fn default_direction() -> String {
    "in".to_string()
}

