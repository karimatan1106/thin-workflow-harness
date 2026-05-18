//! `harness tested-by <qname>` ハンドラ。
//!
//! `--lang auto|rust|ts|py|go` を受け、対応 LSP server を spawn して
//! find_tested_by_for_lang を回し、text/json で stdout 出力。
//! 既定動作は daemon 経由（auto-spawn または `--daemon-port` で固定 port）。
//! 環境変数 `HARNESS_DIRECT_LSP=1` で daemon を bypass し直接 LSP を spawn する（debug 用）。

use std::path::PathBuf;
use std::time::Duration;

use crate::ckg::lsp::tested::TestedNode;
use crate::ckg::lsp::tested_lang::find_tested_by_for_lang;
use crate::handlers_find_symbol::{ensure_server_available, open_client, resolve_lang};
use crate::lsp_daemon::TestedNodePayload;

/// `harness tested-by` CLI ハンドラ。
pub fn cmd_tested_by(
    qname: &str,
    depth: usize,
    root: Option<&str>,
    format: &str,
    lang_arg: &str,
    daemon_port: Option<u16>,
) -> Result<(), String> {
    let root_path = resolve_root(root)?;
    let lang_lazy = || resolve_lang(lang_arg, qname, &root_path);
    let nodes = if let Some(mut c) = open_client(daemon_port, &root_path, &lang_lazy)? {
        let p = c.tested_by(qname, depth, &root_path, Duration::from_secs(120))?;
        p.into_iter().map(tested_payload_to_node).collect()
    } else {
        let lang = lang_lazy()?;
        ensure_server_available(lang)?;
        find_tested_by_for_lang(qname, depth, lang, &root_path)?
    };
    match format {
        "json" => print_json(qname, depth, &nodes)?,
        "text" => print_text(qname, depth, &nodes),
        other => return Err(format!("unknown format: {other} (text|json)")),
    }
    Ok(())
}

fn tested_payload_to_node(p: TestedNodePayload) -> TestedNode {
    TestedNode { name: p.name, file: p.file, line: p.line, depth: p.depth }
}

fn resolve_root(root: Option<&str>) -> Result<PathBuf, String> {
    match root {
        Some(r) => Ok(PathBuf::from(r)),
        None => std::env::current_dir().map_err(|e| format!("cwd: {e}")),
    }
}

fn print_text(qname: &str, depth: usize, nodes: &[TestedNode]) {
    println!("tested-by `{qname}` (depth={depth}):");
    for n in nodes {
        if n.name.is_empty() {
            println!("  d{}: {}:{}", n.depth, n.file, n.line);
        } else {
            println!("  d{}: {} at {}:{}", n.depth, n.name, n.file, n.line);
        }
    }
}

fn print_json(qname: &str, depth: usize, nodes: &[TestedNode]) -> Result<(), String> {
    let payload = serde_json::json!({
        "qname": qname,
        "depth": depth,
        "tests": nodes,
    });
    let s = serde_json::to_string_pretty(&payload).map_err(|e| format!("serialize: {e}"))?;
    println!("{s}");
    Ok(())
}
