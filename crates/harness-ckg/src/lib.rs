//! thin-workflow-harness CKG library crate.
//!
//! tree-sitter ベースの outline / 多言語 LSP wrapper / persistent LSP daemon の
//! protocol + state を library として提供する。binary 側 (harness-lspd) はこれを
//! 依存して CLI primitive (find-symbol / refs / callers / closure / impacted-by /
//! tested-by / outline) と daemon serve/admin を実装する。
//!
//! Phase 2 step 2 で `crates/harness/src/{ckg,lsp_daemon}` から切り出した。

#[cfg(not(windows))]
compile_error!("thin-workflow-harness-ckg is Windows-only");

pub mod ckg;
pub mod lsp_daemon;
