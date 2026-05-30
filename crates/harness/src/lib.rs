//! thin-workflow-harness binary crate ── workflow runner CLI のみ。
//!
//! workflow runner core は thin-workflow-harness-core crate に分離済。
//! CKG / LSP daemon は thin-workflow-harness-ckg (library) +
//! thin-workflow-harness-lspd (binary `harness-lspd`) に分離済。
//!
//! プラットフォーム: Windows / Linux / macOS 対応。

pub mod cli;
pub mod cli_dispatch;
