//! LSP ブリッジ ── subprocess + JSON-RPC framing で言語サーバを同期駆動する。
//!
//! - `framing`: Content-Length ベースの message wire format
//! - `client`: 1 subprocess に対する同期 request/notify ループ
//! - `query`: find_symbol 用ユースケース
//! - `refs` / `refs_parse`: find_refs / find_callers ユースケース（layer 2 拡張）
//! - `closure`: refs/callers の transitive 合成（layer 2 続き）
//! - `impacted`: closure direction=in の薄いラッパ（変更影響範囲）
//! - `tested`: closure direction=in の結果から test 関数だけ抽出
//!
//! 今は rust-analyzer 1 言語のみ。SCIP / プール / 多言語は後続バッチ。

pub mod client;
pub mod closure;
pub mod framing;
pub mod impacted;
pub mod query;
pub mod refs;
mod refs_parse;
pub mod tested;

pub use client::LspClient;
pub use closure::{find_closure, ClosureNode, Direction};
pub use impacted::{find_impacted_by, ImpactedNode};
pub use query::{find_symbol, SymbolInfo};
pub use refs::{find_callers, find_refs, CallerInfo, RefLocation};
pub use tested::{find_tested_by, TestedNode};
