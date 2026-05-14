//! CKG (Code Knowledge Graph) backend ── 最初の slice。
//!
//! - `outline`: tree-sitter ベース（Rust 専用） ── layer 1
//! - `lsp`: LSP ブリッジ（rust-analyzer 専用） ── layer 2
//!
//! 多言語 / SCIP / SQLite ストアは後続バッチ。

pub mod lsp;
pub mod outline;

pub use outline::{outline_file, outline_source, Symbol, SymbolKind};
