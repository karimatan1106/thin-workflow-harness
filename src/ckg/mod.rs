//! CKG (Code Knowledge Graph) backend ── 最初の slice。
//!
//! tree-sitter ベース。今は Rust 言語のみ、`outline_file` を公開。
//! LSP / 多言語 / SCIP は後続バッチ。

pub mod outline;

pub use outline::{outline_file, outline_source, Symbol, SymbolKind};
