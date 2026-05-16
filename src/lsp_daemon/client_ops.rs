//! daemon client convenience methods -- 1 method per op.
//!
//! Each method builds the matching Op variant, sends it, validates `ok=true`,
//! and decodes Response.data into the expected payload Vec.

use std::path::Path;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::client::DaemonClient;
use super::payload::{
    CallerPayload, ClosureNodePayload, RefPayload, SymbolPayload, TestedNodePayload,
};
use super::protocol::{
    CallersParams, ClosureParams, FindSymbolParams, ImpactedByParams, Op, OutgoingParams,
    RefsParams, Response, TestedByParams,
};

fn decode_payload<T: DeserializeOwned>(resp: Response) -> Result<Vec<T>, String> {
    if !resp.ok {
        return Err(resp.error.unwrap_or_else(|| "unknown error".to_string()));
    }
    match resp.data {
        Value::Null => Ok(Vec::new()),
        other => serde_json::from_value(other).map_err(|e| format!("decode payload: {e}")),
    }
}

fn to_root_string(root: &Path) -> String {
    root.to_string_lossy().to_string()
}

impl DaemonClient {
    /// Send `find_symbol` to daemon.
    pub fn find_symbol(
        &mut self,
        qname: &str,
        root: &Path,
        kind: Option<&str>,
        timeout: Duration,
    ) -> Result<Vec<SymbolPayload>, String> {
        let op = Op::FindSymbol(FindSymbolParams {
            qname: qname.to_string(),
            root: to_root_string(root),
            kind: kind.map(|s| s.to_string()),
            timeout_ms: timeout.as_millis() as u64,
        });
        decode_payload(self.send_request_recv_response(op)?)
    }

    /// Send `refs` to daemon.
    pub fn refs(
        &mut self,
        qname: &str,
        root: &Path,
        timeout: Duration,
    ) -> Result<Vec<RefPayload>, String> {
        let op = Op::Refs(RefsParams {
            qname: qname.to_string(),
            root: to_root_string(root),
            timeout_ms: timeout.as_millis() as u64,
        });
        decode_payload(self.send_request_recv_response(op)?)
    }

    /// Send `callers` to daemon.
    pub fn callers(
        &mut self,
        qname: &str,
        root: &Path,
        timeout: Duration,
    ) -> Result<Vec<CallerPayload>, String> {
        let op = Op::Callers(CallersParams {
            qname: qname.to_string(),
            root: to_root_string(root),
            timeout_ms: timeout.as_millis() as u64,
        });
        decode_payload(self.send_request_recv_response(op)?)
    }

    /// Send `outgoing` to daemon.
    pub fn outgoing(
        &mut self,
        qname: &str,
        root: &Path,
        timeout: Duration,
    ) -> Result<Vec<CallerPayload>, String> {
        let op = Op::Outgoing(OutgoingParams {
            qname: qname.to_string(),
            root: to_root_string(root),
            timeout_ms: timeout.as_millis() as u64,
        });
        decode_payload(self.send_request_recv_response(op)?)
    }

    /// Send `closure` to daemon. `direction` is "in" | "out" | "both".
    pub fn closure(
        &mut self,
        qname: &str,
        depth: usize,
        direction: &str,
        root: &Path,
        timeout: Duration,
    ) -> Result<Vec<ClosureNodePayload>, String> {
        let op = Op::Closure(ClosureParams {
            qname: qname.to_string(),
            depth,
            direction: direction.to_string(),
            root: to_root_string(root),
            timeout_ms: timeout.as_millis() as u64,
        });
        decode_payload(self.send_request_recv_response(op)?)
    }

    /// Send `impacted_by` to daemon.
    pub fn impacted_by(
        &mut self,
        qname: &str,
        depth: usize,
        root: &Path,
        timeout: Duration,
    ) -> Result<Vec<TestedNodePayload>, String> {
        let op = Op::ImpactedBy(ImpactedByParams {
            qname: qname.to_string(),
            depth,
            root: to_root_string(root),
            timeout_ms: timeout.as_millis() as u64,
        });
        decode_payload(self.send_request_recv_response(op)?)
    }

    /// Send `tested_by` to daemon.
    pub fn tested_by(
        &mut self,
        qname: &str,
        depth: usize,
        root: &Path,
        timeout: Duration,
    ) -> Result<Vec<TestedNodePayload>, String> {
        let op = Op::TestedBy(TestedByParams {
            qname: qname.to_string(),
            depth,
            root: to_root_string(root),
            timeout_ms: timeout.as_millis() as u64,
        });
        decode_payload(self.send_request_recv_response(op)?)
    }
}
