//! thin-workflow-harness binary crate ── CLI + CKG + LSP daemon
//!
//! workflow runner core は thin-workflow-harness-core crate に分離済。
//! CKG / LSP daemon は次バッチ (Phase 2 step 2) で harness-ckg crate に切り出し予定。

#[cfg(not(windows))]
compile_error!("thin-workflow-harness is Windows-only");

pub mod ckg;
pub mod cli;
pub mod cli_daemon;
pub mod cli_dispatch;
pub mod cli_query;
pub mod handlers_closure;
pub mod handlers_find_symbol;
pub mod handlers_impacted;
pub mod handlers_outline;
pub mod handlers_refs;
pub mod handlers_tested;
pub mod lsp_daemon;
