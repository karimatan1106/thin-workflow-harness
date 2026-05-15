//! Lang 引数版の `find_symbol`。既存 `query::find_symbol(server_cmd, ...)` の
//! 多言語ラッパで、`Lang` から server コマンドを解決して spawn する。
//!
//! 200 行制約のため query.rs から分離。

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::client::LspClient;
use super::lang::{lsp_server_cmd, Lang};
use super::query::{parse_workspace_symbols, path_to_file_uri, SymbolInfo};

/// `find_symbol` の Lang 版。
///
/// - Rust → rust-analyzer
/// - Ts   → typescript-language-server --stdio
///
/// LSP request は `workspace/symbol` で同じ。各 server が文法差を吸収する。
pub fn find_symbol_for_lang(
    lang: Lang,
    root: &Path,
    query: &str,
    kind_filter: Option<&str>,
    timeout: Duration,
) -> Result<Vec<SymbolInfo>, String> {
    let (cmd, _args) = lsp_server_cmd(lang);
    let mut client = LspClient::start_for_lang(lang)
        .map_err(|e| format!("spawn {cmd}: {e}"))?;
    let root_uri = path_to_file_uri(root)?;
    let _ = client.initialize(&root_uri)?;

    let started = Instant::now();
    let mut symbols: Vec<SymbolInfo>;
    loop {
        let resp: Value =
            client.request("workspace/symbol", json!({ "query": query }))?;
        symbols = parse_workspace_symbols(&resp);
        if !symbols.is_empty() {
            break;
        }
        if started.elapsed() >= timeout {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    let filtered = match kind_filter {
        Some(k) => symbols
            .into_iter()
            .filter(|s| s.kind.eq_ignore_ascii_case(k))
            .collect(),
        None => symbols,
    };

    let _ = client.shutdown();
    Ok(filtered)
}
