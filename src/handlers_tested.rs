//! `harness tested-by <qname>` ハンドラ。
//!
//! rust-analyzer を spawn して find_tested_by を 1 回回し、text/json で stdout 出力。
//! find_closure(direction=in) の結果から test 関数のみフィルタする。

use std::path::PathBuf;
use std::time::Duration;

use crate::ckg::lsp::tested::{find_tested_by, TestedNode};
use crate::handlers_find_symbol::resolve_server_cmd;

/// `harness tested-by` CLI ハンドラ。
pub fn cmd_tested_by(
    qname: &str,
    depth: usize,
    root: Option<&str>,
    format: &str,
) -> Result<(), String> {
    let server_cmd = resolve_server_cmd()?;
    let root_path = resolve_root(root)?;
    let timeout = Duration::from_secs(60);
    let nodes = find_tested_by(&server_cmd, &root_path, qname, depth, timeout)?;
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
