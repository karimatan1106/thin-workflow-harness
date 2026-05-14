//! 上位ユースケース API ── 今は `find_symbol(query, root, kind?)` だけ。
//!
//! 内部で `LspClient` を spawn → initialize → workspace/symbol → shutdown までやる。
//! rust-analyzer は indexing 中は `workspace/symbol` が空配列を返すので、
//! 少しずつ間隔を空けて短いリトライを掛ける。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use super::client::LspClient;

/// `find_symbol` が返す 1 件分。最小限のフィールドだけ。
#[derive(Debug, Clone, Serialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
    pub col: usize,
}

/// `harness find-symbol <query>` の実体。
///
/// - `server_cmd`: LSP サーバ（"rust-analyzer" 等）
/// - `root`: workspace ルート（絶対パス推奨）
/// - `query`: 検索文字列
/// - `kind_filter`: 例 `Some("function")` → fn のみ。`None` は全種別。
/// - `timeout`: indexing 待ち上限。これを超えたら最後に得た（空かもしれない）配列を返す。
pub fn find_symbol(
    server_cmd: &str,
    root: &Path,
    query: &str,
    kind_filter: Option<&str>,
    timeout: Duration,
) -> Result<Vec<SymbolInfo>, String> {
    let mut client = LspClient::spawn(server_cmd)?;
    let root_uri = path_to_file_uri(root)?;
    let _ = client.initialize(&root_uri)?;

    let started = Instant::now();
    let mut symbols: Vec<SymbolInfo>;
    loop {
        let resp: Value = client.request(
            "workspace/symbol",
            json!({ "query": query }),
        )?;
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

/// `WorkspaceSymbolResponse` は SymbolInformation[] または WorkspaceSymbol[] どちらでも来る。
/// どちらでも対応できるよう Value からゆるく拾う。
fn parse_workspace_symbols(v: &Value) -> Vec<SymbolInfo> {
    let arr = match v.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let name = item
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let kind_num = item.get("kind").and_then(|x| x.as_u64()).unwrap_or(0) as u8;
        let kind = symbol_kind_name(kind_num);
        let (file, line, col) = extract_location(item);
        out.push(SymbolInfo { name, kind, file, line, col });
    }
    out
}

fn extract_location(item: &Value) -> (String, usize, usize) {
    // SymbolInformation: { location: { uri, range: { start } } }
    // WorkspaceSymbol:    { location: { uri } } もしくは { location: Location }
    let loc = item.get("location").cloned().unwrap_or(Value::Null);
    let uri = loc.get("uri").and_then(|x| x.as_str()).unwrap_or("");
    let file = uri_to_path_string(uri);
    let start = loc
        .get("range")
        .and_then(|r| r.get("start"))
        .cloned()
        .unwrap_or(Value::Null);
    let line = start.get("line").and_then(|x| x.as_u64()).unwrap_or(0) as usize + 1;
    let col = start.get("character").and_then(|x| x.as_u64()).unwrap_or(0) as usize + 1;
    (file, line, col)
}

/// LSP `SymbolKind` 数値 → 文字列。
/// 参照: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#symbolKind
fn symbol_kind_name(n: u8) -> String {
    let s = match n {
        1 => "file",
        2 => "module",
        3 => "namespace",
        4 => "package",
        5 => "class",
        6 => "method",
        7 => "property",
        8 => "field",
        9 => "constructor",
        10 => "enum",
        11 => "interface",
        12 => "function",
        13 => "variable",
        14 => "constant",
        15 => "string",
        16 => "number",
        17 => "boolean",
        18 => "array",
        19 => "object",
        20 => "key",
        21 => "null",
        22 => "enum_member",
        23 => "struct",
        24 => "event",
        25 => "operator",
        26 => "type_parameter",
        _ => "unknown",
    };
    s.to_string()
}

/// 絶対パス → `file:///...` URI（粗いが LSP 実装はだいたい寛容）。
pub(super) fn path_to_file_uri(p: &Path) -> Result<String, String> {
    let abs: PathBuf = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("cwd: {e}"))?
            .join(p)
    };
    let s = abs.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        Ok(format!("file://{s}"))
    } else {
        Ok(format!("file:///{s}"))
    }
}

/// `file:///C:/a/b` → `C:/a/b`、`file:///home/x` → `/home/x` （表示用、ゆるい）。
pub(super) fn uri_to_path_string(uri: &str) -> String {
    if let Some(rest) = uri.strip_prefix("file://") {
        let trimmed = rest.trim_start_matches('/');
        // Windows: `C:/...` で始まるなら頭の / は不要、それ以外（POSIX）は / を付け直す。
        if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
            return trimmed.to_string();
        }
        return format!("/{trimmed}");
    }
    uri.to_string()
}
