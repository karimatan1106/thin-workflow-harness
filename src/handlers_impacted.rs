//! `harness impacted-by <qname>` ハンドラ。
//!
//! rust-analyzer を spawn して find_impacted_by を 1 回回し、text/json で stdout 出力。
//! 内部は find_closure(direction=in) の薄いラッパ。

use std::path::PathBuf;
use std::time::Duration;

use crate::ckg::lsp::impacted::{find_impacted_by, ImpactedNode};
use crate::handlers_find_symbol::resolve_server_cmd;

/// `harness impacted-by` CLI ハンドラ。
pub fn cmd_impacted_by(
    qname: &str,
    depth: usize,
    root: Option<&str>,
    format: &str,
) -> Result<(), String> {
    let server_cmd = resolve_server_cmd()?;
    let root_path = resolve_root(root)?;
    let timeout = Duration::from_secs(60);
    let nodes = find_impacted_by(&server_cmd, &root_path, qname, depth, timeout)?;
    match format {
        "json" => print_json(qname, depth, &nodes)?,
        "text" => print_text(qname, depth, &nodes),
        other => return Err(format!("unknown format: {other} (text|json)")),
    }
    Ok(())
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
