//! LSP ブリッジ ── subprocess + JSON-RPC framing で言語サーバを同期駆動する。
//!
//! - `framing`: Content-Length ベースの message wire format
//! - `client`: 1 subprocess に対する同期 request/notify ループ
//! - `query`: 上位ユースケース（find_symbol 等）
//!
//! 今は rust-analyzer 1 言語のみ。SCIP / プール / 多言語は後続バッチ。

pub mod client;
pub mod framing;
pub mod query;

pub use client::LspClient;
pub use query::{find_symbol, SymbolInfo};
