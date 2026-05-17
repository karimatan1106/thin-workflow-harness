//! daemon request dispatch -- match each Op and route to per-op primitive in
//! `dispatch_ops`. Health は in-place で snapshot を返し、LSP 統計を汚さない。
//!
//! - handle_request: Op match + 時間計測 (health は除外)
//! - do_health: state.snapshot() → HealthPayload
//! - lock / ok_response / err_response: helper、`dispatch_ops` から再利用

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use serde_json::Value;

use crate::ckg::lsp::{Lang, LspClient};

use super::dispatch_ops::{
    do_callers, do_closure, do_find_symbol, do_impacted_by, do_outgoing, do_refs,
    do_tested_by,
};
use super::payload::HealthPayload;
use super::protocol::{Op, Request, Response};
use super::state::DaemonState;

/// Dispatch one parsed request to the matching primitive. The shared LspClient
/// is locked for the duration of the call. health は計測対象外 (queries_handled
/// は LSP query 数のみカウント、health 自身を含めると noisy)。
pub fn handle_request(
    client: &Arc<Mutex<LspClient>>,
    state: &Arc<DaemonState>,
    lang: Lang,
    _root: &Path,
    req: Request,
) -> Response {
    let id = req.id;
    if let Op::Health(_) = &req.op {
        return do_health(id, state);
    }
    let t = Instant::now();
    let resp = match req.op {
        Op::FindSymbol(p) => do_find_symbol(client, lang, id, p),
        Op::Refs(p) => do_refs(client, lang, id, p),
        Op::Callers(p) => do_callers(client, lang, id, p),
        Op::Outgoing(p) => do_outgoing(client, lang, id, p),
        Op::Closure(p) => do_closure(client, lang, id, p),
        Op::ImpactedBy(p) => do_impacted_by(client, lang, id, p),
        Op::TestedBy(p) => do_tested_by(client, lang, id, p),
        Op::Health(_) => unreachable!("health handled above"),
    };
    state.record_query(t.elapsed().as_millis() as u64);
    resp
}

fn do_health(id: u64, state: &Arc<DaemonState>) -> Response {
    let snap = state.snapshot();
    let payload = HealthPayload {
        status: "ready".to_string(),
        lang: snap.lang,
        uptime_ms: snap.uptime_ms,
        queries_handled: snap.queries_handled,
        recent_avg_ms: snap.recent_avg_ms,
    };
    ok_response(id, payload)
}

pub(super) fn lock<'a>(
    c: &'a Arc<Mutex<LspClient>>,
) -> std::sync::MutexGuard<'a, LspClient> {
    match c.lock() {
        Ok(g) => g,
        Err(poison) => poison.into_inner(),
    }
}

pub(super) fn ok_response<T: Serialize>(id: u64, payload: T) -> Response {
    Response {
        id,
        ok: true,
        data: serde_json::to_value(&payload).unwrap_or(Value::Null),
        error: None,
    }
}

pub(super) fn err_response(id: u64, e: String) -> Response {
    Response { id, ok: false, data: Value::Null, error: Some(e) }
}
