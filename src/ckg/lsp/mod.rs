//! LSP ブリッジ ── subprocess + JSON-RPC framing で言語サーバを同期駆動する。
//!
//! - `framing`: Content-Length ベースの message wire format
//! - `client`: 1 subprocess に対する同期 request/notify ループ
//! - `query`: find_symbol 用ユースケース
//! - `refs` / `refs_parse`: find_refs / find_callers ユースケース（layer 2 拡張）
//! - `closure`: refs/callers の transitive 合成（layer 2 続き）
//!
//! 今は rust-analyzer 1 言語のみ。SCIP / プール / 多言語は後続バッチ。

pub mod client;
pub mod closure;
pub mod framing;
pub mod query;
pub mod refs;
mod refs_parse;

pub use client::LspClient;
pub use closure::{find_closure, ClosureNode, Direction};
pub use query::{find_symbol, SymbolInfo};
pub use refs::{find_callers, find_refs, CallerInfo, RefLocation};
