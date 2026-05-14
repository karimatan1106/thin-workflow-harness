//! CKG (Code Knowledge Graph) backend ── 最初の slice。
//!
//! - `outline`: tree-sitter ベース（Rust 専用） ── layer 1
//! - `lsp`: LSP ブリッジ（rust-analyzer 専用） ── layer 2
//! - `test_attrs`: `#[test]` 系 attr 検出 ── tested-by 精度向上用
//!
//! 多言語 / SCIP / SQLite ストアは後続バッチ。

pub mod lsp;
pub mod outline;
pub mod test_attrs;

pub use outline::{outline_file, outline_source, Symbol, SymbolKind};
pub use test_attrs::{is_test_function, list_test_function_lines};
