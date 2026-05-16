//! `harness impacted-by <qname>` ハンドラ。
//!
//! `--lang auto|rust|ts|py|go` を受け、対応 LSP server を spawn して
//! find_impacted_by_for_lang を回し、text/json で stdout 出力。
//! 内部は find_closure_for_lang(direction=in) の薄いラッパ。
//! `--daemon-port <port>` / `--use-daemon` で layer 2.5 daemon 経由可。

use std::path::PathBuf;
use std::time::Duration;

use crate::ckg::lsp::impacted::{find_impacted_by_for_lang, ImpactedNode};
use crate::handlers_find_symbol::{ensure_server_available, open_client, resolve_lang};
use crate::lsp_daemon::TestedNodePayload;

/// `harness impacted-by` CLI ハンドラ。
pub fn cmd_impacted_by(
    qname: &str,
    depth: usize,
    root: Option<&str>,
    format: &str,
    lang_arg: &str,
    daemon_port: Option<u16>,
    use_daemon: bool,
) -> Result<(), String> {
    let root_path = resolve_root(root)?;
    let lang_lazy = || resolve_lang(lang_arg, qname, &root_path);
    let nodes = if let Some(mut c) = open_client(daemon_port, use_daemon, &root_path, &lang_lazy)? {
        let p = c.impacted_by(qname, depth, &root_path, Duration::from_secs(120))?;
        p.into_iter().map(tested_payload_to_impacted).collect()
    } else {
        let lang = lang_lazy()?;
        ensure_server_available(lang)?;
        find_impacted_by_for_lang(qname, depth, lang, &root_path)?
    };
    match format {
        "json" => print_json(qname, depth, &nodes)?,
        "text" => print_text(qname, depth, &nodes),
        other => return Err(format!("unknown format: {other} (text|json)")),
    }
    Ok(())
}

fn tested_payload_to_impacted(p: TestedNodePayload) -> ImpactedNode {
    ImpactedNode { name: p.name, file: p.file, line: p.line, depth: p.depth }
}

fn resolve_root(root: Option<&str>) -> Result<PathBuf, String> {
    match root {
        Some(r) => Ok(PathBuf::from(r)),
        None => std::env::current_dir().map_err(|e| format!("cwd: {e}")),
    }
}

fn print_text(qname: &str, depth: usize, nodes: &[ImpactedNode]) {
    println!("impacted-by `{qname}` (depth={depth}):");
    for n in nodes {
        if n.name.is_empty() {
            println!("  d{}: {}:{}", n.depth, n.file, n.line);
        } else {
            println!("  d{}: {} at {}:{}", n.depth, n.name, n.file, n.line);
        }
    }
}

fn print_json(qname: &str, depth: usize, nodes: &[ImpactedNode]) -> Result<(), String> {
    let payload = serde_json::json!({
        "qname": qname,
        "depth": depth,
        "impacted": nodes,
    });
    let s = serde_json::to_string_pretty(&payload).map_err(|e| format!("serialize: {e}"))?;
    println!("{s}");
    Ok(())
}
