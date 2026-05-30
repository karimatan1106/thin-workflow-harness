//! thin-workflow-harness LSP daemon + CKG primitive CLI binary crate.
//!
//! harness-ckg crate を library 依存にして、harness-lspd.exe 用の CLI primitive
//! (find-symbol / refs / callers / closure / impacted-by / tested-by / outline /
//! query / lsp-daemon serve/list/stop/health) を提供する。
//!
//! Phase 2 step 3 で `crates/harness/src/{handlers_*,cli_query,cli_daemon}` から
//! 切り出した。workflow runner core は thin-workflow-harness-core / debug CLI と
//! workflow 系コマンドは thin-workflow-harness binary 側に残る。
//!
//! プラットフォーム: Windows / Linux / macOS 対応（本体 crate に合わせ Windows-only ガードは撤去）。

pub mod cli;
pub mod cli_daemon;
pub mod cli_query;
pub mod handlers_closure;
pub mod handlers_find_symbol;
pub mod handlers_impacted;
pub mod handlers_outline;
pub mod handlers_refs;
pub mod handlers_tested;
