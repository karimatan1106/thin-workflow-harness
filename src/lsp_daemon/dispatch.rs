//! daemon request dispatch -- match each Op and call the matching
//! `find_*_for_lang_with_client` primitive, then serialize the result into
//! Response.data as JSON.
//!
//! Splits a single function per op for readability + future per-op fan-out.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::ckg::lsp::{
    find_callers_for_lang_with_client, find_closure_for_lang_with_client,
    find_impacted_by_for_lang_with_client, find_outgoing_for_lang_with_client,
    find_refs_for_lang_with_client, find_symbol_for_lang_with_client,
    find_tested_by_for_lang_with_client, Direction, Lang, LspClient,
};

use super::payload::{
    CallerPayload, ClosureNodePayload, RefPayload, SymbolPayload, TestedNodePayload,
};
use super::protocol::{
    CallersParams, ClosureParams, FindSymbolParams, ImpactedByParams, Op, OutgoingParams,
    RefsParams, Request, Response, TestedByParams,
};

/// Dispatch one parsed request to the matching primitive. The shared LspClient
/// is locked for the duration of the call.
pub fn handle_request(
    client: &Arc<Mutex<LspClient>>,
    lang: Lang,
    _root: &Path,
    req: Request,
) -> Response {
    let id = req.id;
    match req.op {
        Op::FindSymbol(p) => do_find_symbol(client, lang, id, p),
        Op::Refs(p) => do_refs(client, lang, id, p),
        Op::Callers(p) => do_callers(client, lang, id, p),
        Op::Outgoing(p) => do_outgoing(client, lang, id, p),
        Op::Closure(p) => do_closure(client, lang, id, p),
        Op::ImpactedBy(p) => do_impacted_by(client, lang, id, p),
        Op::TestedBy(p) => do_tested_by(client, lang, id, p),
    }
}

fn lock<'a>(c: &'a Arc<Mutex<LspClient>>) -> std::sync::MutexGuard<'a, LspClient> {
    match c.lock() {
        Ok(g) => g,
        Err(poison) => poison.into_inner(),
    }
}

fn ok_response<T: Serialize>(id: u64, payload: T) -> Response {
    Response {
        id,
        ok: true,
        data: serde_json::to_value(&payload).unwrap_or(Value::Null),
        error: None,
    }
}

fn err_response(id: u64, e: String) -> Response {
    Response { id, ok: false, data: Value::Null, error: Some(e) }
}

fn do_find_symbol(
    client: &Arc<Mutex<LspClient>>,
    lang: Lang,
    id: u64,
    p: FindSymbolParams,
) -> Response {
    let root = PathBuf::from(&p.root);
    let timeout = Duration::from_millis(p.timeout_ms);
    let mut g = lock(client);
    match find_symbol_for_lang_with_client(
        &mut g,
        lang,
        &root,
        &p.qname,
        p.kind.as_deref(),
        timeout,
    ) {
        Ok(syms) => {
            let payload: Vec<SymbolPayload> =
                syms.into_iter().map(SymbolPayload::from).collect();
            ok_response(id, payload)
        }
        Err(e) => err_response(id, e),
    }
}

fn do_refs(client: &Arc<Mutex<LspClient>>, lang: Lang, id: u64, p: RefsParams) -> Response {
    let root = PathBuf::from(&p.root);
    let timeout = Duration::from_millis(p.timeout_ms);
    let mut g = lock(client);
    match find_refs_for_lang_with_client(&mut g, lang, &root, &p.qname, timeout) {
        Ok(refs) => {
            let payload: Vec<RefPayload> = refs.into_iter().map(RefPayload::from).collect();
            ok_response(id, payload)
        }
        Err(e) => err_response(id, e),
    }
}

fn do_callers(
    client: &Arc<Mutex<LspClient>>,
    lang: Lang,
    id: u64,
    p: CallersParams,
) -> Response {
    let root = PathBuf::from(&p.root);
    let timeout = Duration::from_millis(p.timeout_ms);
    let mut g = lock(client);
    match find_callers_for_lang_with_client(&mut g, lang, &root, &p.qname, timeout) {
        Ok(v) => {
            let payload: Vec<CallerPayload> = v.into_iter().map(CallerPayload::from).collect();
            ok_response(id, payload)
        }
        Err(e) => err_response(id, e),
    }
}

fn do_outgoing(
    client: &Arc<Mutex<LspClient>>,
    lang: Lang,
    id: u64,
    p: OutgoingParams,
) -> Response {
    let root = PathBuf::from(&p.root);
    let timeout = Duration::from_millis(p.timeout_ms);
    let mut g = lock(client);
    match find_outgoing_for_lang_with_client(&mut g, lang, &root, &p.qname, timeout) {
        Ok(v) => {
            let payload: Vec<CallerPayload> = v.into_iter().map(CallerPayload::from).collect();
            ok_response(id, payload)
        }
        Err(e) => err_response(id, e),
    }
}

fn do_closure(
    client: &Arc<Mutex<LspClient>>,
    lang: Lang,
    id: u64,
    p: ClosureParams,
) -> Response {
    let root = PathBuf::from(&p.root);
    let dir = match Direction::parse(&p.direction) {
        Ok(d) => d,
        Err(e) => return err_response(id, e),
    };
    let mut g = lock(client);
    match find_closure_for_lang_with_client(&mut g, &p.qname, p.depth, dir, lang, &root) {
        Ok(v) => {
            let payload: Vec<ClosureNodePayload> =
                v.into_iter().map(ClosureNodePayload::from).collect();
            ok_response(id, payload)
        }
        Err(e) => err_response(id, e),
    }
}

fn do_impacted_by(
    client: &Arc<Mutex<LspClient>>,
    lang: Lang,
    id: u64,
    p: ImpactedByParams,
) -> Response {
    let root = PathBuf::from(&p.root);
    let mut g = lock(client);
    match find_impacted_by_for_lang_with_client(&mut g, &p.qname, p.depth, lang, &root) {
        Ok(v) => {
            let payload: Vec<TestedNodePayload> =
                v.into_iter().map(TestedNodePayload::from).collect();
            ok_response(id, payload)
        }
        Err(e) => err_response(id, e),
    }
}

fn do_tested_by(
    client: &Arc<Mutex<LspClient>>,
    lang: Lang,
    id: u64,
    p: TestedByParams,
) -> Response {
    let root = PathBuf::from(&p.root);
    let mut g = lock(client);
    match find_tested_by_for_lang_with_client(&mut g, &p.qname, p.depth, lang, &root) {
        Ok(v) => {
            let payload: Vec<TestedNodePayload> =
                v.into_iter().map(TestedNodePayload::from).collect();
            ok_response(id, payload)
        }
        Err(e) => err_response(id, e),
    }
}
