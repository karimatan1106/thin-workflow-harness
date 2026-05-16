//! Lang 引数版の `find_symbol`。layer 2.5 PoC で `_with_client` 版を分離。
//! 既存 API は内部で `_with_client` を呼ぶ薄いラッパに退避。
//! 200 行制約のため query.rs から分離。

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::client::{start_and_warm_up, LspClient};
use super::lang::Lang;
use super::query::{parse_workspace_symbols, SymbolInfo};

/// `find_symbol` の Lang 版 (既存 fire-and-forget API)。
pub fn find_symbol_for_lang(
    lang: Lang,
    root: &Path,
    query: &str,
    kind_filter: Option<&str>,
    timeout: Duration,
) -> Result<Vec<SymbolInfo>, String> {
    let mut client = start_and_warm_up(lang, root)?;
    let result = find_symbol_for_lang_with_client(
        &mut client, lang, root, query, kind_filter, timeout,
    );
    let _ = client.shutdown();
    result
}

/// `find_symbol_for_lang` の client 再利用版。呼び出し側は warm-up 済み
/// `LspClient` を渡す前提。複数 query 連投の hot path で使う。
pub fn find_symbol_for_lang_with_client(
    client: &mut LspClient,
    _lang: Lang,
    _root: &Path,
    query: &str,
    kind_filter: Option<&str>,
    timeout: Duration,
) -> Result<Vec<SymbolInfo>, String> {
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
    Ok(filtered)
}
