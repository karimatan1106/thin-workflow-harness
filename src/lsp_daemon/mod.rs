//! LSP daemon (layer 2.5) -- foreground TCP localhost daemon.
//!
//! - protocol: line-based JSON over TCP wire format (Request/Response/Op enum)
//! - payload: response data shapes (SymbolPayload/RefPayload/CallerPayload/...)
//! - server: run_daemon(lang, root, port) drives 1 LspClient + listener
//! - dispatch: op -> find_*_for_lang_with_client() routing
//! - client: DaemonClient::connect(port) opens TCP + shared send/recv
//! - client_ops: per-op convenience methods on DaemonClient
//!
//! Scope: foreground / 7 ops (find_symbol + refs + callers + outgoing + closure
//! + impacted_by + tested_by) / single Lang per daemon.

pub mod client;
pub mod client_ops;
pub mod dispatch;
pub mod payload;
pub mod protocol;
pub mod server;

#[cfg(test)]
mod protocol_tests;

pub use client::DaemonClient;
pub use payload::{
    CallerPayload, ClosureNodePayload, RefPayload, SymbolPayload, TestedNodePayload,
};
pub use protocol::{
    CallersParams, ClosureParams, FindSymbolParams, ImpactedByParams, Op, OutgoingParams,
    RefsParams, Request, Response, TestedByParams,
};
pub use server::run_daemon;
