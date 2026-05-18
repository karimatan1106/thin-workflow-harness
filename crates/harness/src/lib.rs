//! thin-workflow-harness binary crate ── workflow runner CLI のみ。
//!
//! workflow runner core は thin-workflow-harness-core crate に分離済。
//! CKG / LSP daemon は thin-workflow-harness-ckg (library) +
//! thin-workflow-harness-lspd (binary `harness-lspd`) に分離済。

#[cfg(not(windows))]
compile_error!("thin-workflow-harness is Windows-only");

pub mod cli;
pub mod cli_dispatch;
