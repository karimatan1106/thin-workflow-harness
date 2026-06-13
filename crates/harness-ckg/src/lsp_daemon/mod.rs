//! LSP daemon (layer 2.5) -- foreground TCP localhost daemon.
//!
//! - protocol: line-based JSON over TCP wire format (Request/Response/Op enum)
//! - payload: response data shapes (SymbolPayload/RefPayload/CallerPayload/...)
//! - server: run_daemon(lang, root, port) drives 1 LspClient + listener
//! - dispatch: op -> find_*_for_lang_with_client() routing
//! - client: DaemonClient::connect(port) / connect_or_spawn(lang, root, timeout)
//! - client_ops: per-op convenience methods on DaemonClient
//! - port_file: ~/.cache/thin-workflow-harness/daemon-<lang>-<hash>.port 規約
//! - port_file_list: cache_dir enumerate (admin/list 用)
//! - admin: `harness lsp-daemon list/stop` 実装
//!
//! Scope: foreground / 7 ops (find_symbol + refs + callers + outgoing + closure
//! + impacted_by + tested_by) / single Lang per daemon / auto-spawn from client.

pub mod admin;
pub mod client;
pub mod client_ops;
pub mod dispatch;
pub mod dispatch_ops;
pub mod payload;
pub mod port_file;
pub mod port_file_list;
pub mod protocol;
pub mod server;
pub mod state;

#[cfg(test)]
mod protocol_tests;

pub use client::DaemonClient;
pub use payload::{
    CallerPayload, ClosureNodePayload, HealthPayload, RefPayload, SymbolPayload,
    TestedNodePayload,
};
pub use protocol::{
    CallersParams, ClosureParams, FindSymbolParams, HealthParams, ImpactedByParams, Op,
    OutgoingParams, RefsParams, Request, Response, TestedByParams,
};
pub use server::run_daemon;
