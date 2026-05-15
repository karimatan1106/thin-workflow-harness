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
//! 多言語 LSP の足場として `lang` / `query_lang` を追加。
//! 現状 query (find_symbol) のみ Lang 受付。refs / callers / closure 等は別バッチ。

pub mod client;
pub mod closure;
pub mod framing;
pub mod impacted;
pub mod lang;
pub mod query;
pub mod query_lang;
pub mod refs;
mod refs_parse;
pub mod tested;
pub mod uri;

pub use client::LspClient;
pub use closure::{find_closure, ClosureNode, Direction};
pub use impacted::{find_impacted_by, ImpactedNode};
pub use lang::{detect_lang, detect_lang_from_qname, lsp_server_cmd, root_lang, Lang};
pub use query::{find_symbol, SymbolInfo};
pub use query_lang::find_symbol_for_lang;
pub use refs::{find_callers, find_refs, CallerInfo, RefLocation};
pub use tested::{find_tested_by, TestedNode};
