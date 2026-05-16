//! LSP daemon (layer 2.5) -- foreground TCP localhost daemon.
//!
//! - protocol: line-based JSON over TCP wire format
//! - server: run_daemon(lang, root, port) drives 1 LSP client + listener
//! - client: DaemonClient::connect(port) talks to daemon
//!
//! PoC scope: foreground / find_symbol only / Rust 1 lang / single client reuse.

pub mod client;
pub mod protocol;
pub mod server;

pub use client::DaemonClient;
pub use protocol::{FindSymbolParams, Op, Request, Response, SymbolPayload};
pub use server::run_daemon;
