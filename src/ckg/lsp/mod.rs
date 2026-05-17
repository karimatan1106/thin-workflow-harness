//! LSP ブリッジ ── subprocess + JSON-RPC framing で言語サーバを同期駆動する。
//!
//! - `framing`: Content-Length ベースの message wire format
//! - `client`: 1 subprocess に対する同期 request/notify ループ
//! - `query`: find_symbol 用ユースケース
//! - `refs` / `refs_parse`: find_refs / find_callers ユースケース（layer 2 拡張）
//! - `closure` / `closure_lang`: refs/callers の transitive 合成（多言語対応）
//! - `impacted`: closure direction=in の薄いラッパ（変更影響範囲、Lang 対応）
//! - `tested` / `tested_lang`: closure direction=in の結果から test 関数だけ抽出
//!
//! 多言語 LSP の足場として `lang` / `query_lang` / `refs_lang` /
//! `closure_lang` / `tested_lang` を分離配置。各 query primitive が Lang 受付。

pub mod client;
mod client_spawn;
pub mod closure;
pub mod closure_lang;
pub mod framing;
pub mod impacted;
pub mod init_options;
pub mod lang;
pub mod query;
pub mod query_lang;
pub mod refs;
pub mod refs_lang;
mod refs_parse;
pub mod tested;
pub mod tested_go;
pub mod tested_lang;
pub mod tested_py;
pub mod tested_rust;
pub mod tested_ts;
pub mod ts_bootstrap;
pub mod uri;

pub use client::{start_and_warm_up, LspClient};
pub use closure::{find_closure, ClosureNode, Direction};
pub use closure_lang::{find_closure_for_lang, find_closure_for_lang_with_client};
pub use impacted::{
    find_impacted_by, find_impacted_by_for_lang, find_impacted_by_for_lang_with_client,
    ImpactedNode,
};
pub use lang::{detect_lang, detect_lang_from_qname, lsp_server_cmd, root_lang, Lang};
pub use query::{find_symbol, SymbolInfo};
pub use query_lang::{find_symbol_for_lang, find_symbol_for_lang_with_client};
pub use refs::{find_callers, find_refs, CallerInfo, RefLocation};
pub use refs_lang::{
    find_callers_for_lang, find_callers_for_lang_with_client, find_outgoing_for_lang,
    find_outgoing_for_lang_with_client, find_refs_for_lang, find_refs_for_lang_with_client,
};
pub use tested::{find_tested_by, TestedNode};
pub use tested_lang::{find_tested_by_for_lang, find_tested_by_for_lang_with_client};
