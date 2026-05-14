//! `harness closure <qname>` ハンドラ。
//!
//! rust-analyzer を spawn して find_closure を 1 回回し、text/json で stdout 出力。
//! rust-analyzer が PATH に無ければエラー。

use std::path::PathBuf;
use std::time::Duration;

use crate::ckg::lsp::{find_closure, ClosureNode, Direction};
use crate::handlers_find_symbol::resolve_server_cmd;

/// `harness closure` CLI ハンドラ。
pub fn cmd_closure(
    qname: &str,
    depth: usize,
    direction: &str,
    root: Option<&str>,
    format: &str,
) -> Result<(), String> {
    let server_cmd = resolve_server_cmd()?;
    let root_path = resolve_root(root)?;
    let dir = Direction::parse(direction)?;
    let timeout = Duration::from_secs(60);
    let nodes = find_closure(&server_cmd, &root_path, qname, depth, dir, timeout)?;
    match format {
        "json" => print_json(qname, depth, dir, &nodes)?,
        "text" => print_text(qname, depth, dir, &nodes),
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

fn print_text(qname: &str, depth: usize, dir: Direction, nodes: &[ClosureNode]) {
    println!(
        "closure of `{qname}` (direction={dir}, depth={depth}):",
        dir = dir.as_str()
    );
    for n in nodes {
        if n.name.is_empty() {
            println!("  d{}: {}:{}", n.depth, n.file, n.line);
        } else {
            println!("  d{}: {} at {}:{}", n.depth, n.name, n.file, n.line);
        }
    }
}

fn print_json(
    qname: &str,
    depth: usize,
    dir: Direction,
    nodes: &[ClosureNode],
) -> Result<(), String> {
    let payload = serde_json::json!({
        "qname": qname,
        "direction": dir.as_str(),
        "depth": depth,
        "nodes": nodes,
    });
    let s = serde_json::to_string_pretty(&payload).map_err(|e| format!("serialize: {e}"))?;
    println!("{s}");
    Ok(())
}
