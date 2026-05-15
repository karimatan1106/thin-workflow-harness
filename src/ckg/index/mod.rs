//! CKG layer 3 ── SCIP + SQLite キャッシュ層。
//!
//! Phase A1: SQLite skeleton + schema + `harness index init` CLI。
//! Phase A2 以降:
//! - SCIP loader (rust-analyzer scip → symbols / refs / call_edges)
//! - find-symbol を SQLite 先行 + LSP fallback に拡張

pub mod db;
pub mod schema;

pub use db::{IndexDb, SymbolRow};
pub use schema::{migrate, SCHEMA_VERSION};
