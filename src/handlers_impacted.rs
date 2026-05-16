//! `harness impacted-by <qname>` ハンドラ。
//!
//! `--lang auto|rust|ts|py|go` を受け、対応 LSP server を spawn して
//! find_impacted_by_for_lang を回し、text/json で stdout 出力。
//! 内部は find_closure_for_lang(direction=in) の薄いラッパ。
//! `--daemon-port <port>` 指定時は LSP 直接 spawn を bypass、layer 2.5 daemon に投げる。

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::ckg::lsp::impacted::{find_impacted_by_for_lang, ImpactedNode};
use crate::handlers_find_symbol::{ensure_server_available, resolve_lang};
use crate::lsp_daemon::{DaemonClient, TestedNodePayload};

/// `harness impacted-by` CLI ハンドラ。
pub fn cmd_impacted_by(
    qname: &str,
    depth: usize,
    root: Option<&str>,
    format: &str,
    lang_arg: &str,
    daemon_port: Option<u16>,
) -> Result<(), String> {
    let root_path = resolve_root(root)?;
    let nodes = if let Some(port) = daemon_port {
        fetch_impacted_via_daemon(port, qname, depth, &root_path)?
    } else {
        let lang = resolve_lang(lang_arg, qname, &root_path)?;
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

fn fetch_impacted_via_daemon(
    port: u16,
    qname: &str,
    depth: usize,
    root: &Path,
) -> Result<Vec<ImpactedNode>, String> {
    let mut client = DaemonClient::connect(port)?;
    let payload = client.impacted_by(qname, depth, root, Duration::from_secs(120))?;
    Ok(payload.into_iter().map(tested_payload_to_impacted).collect())
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
