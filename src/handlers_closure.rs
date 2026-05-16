//! `harness closure <qname>` ハンドラ。
//!
//! `--lang auto|rust|ts|py|go` を受け、対応 LSP server (rust-analyzer /
//! typescript-language-server / pyright) を spawn して find_closure_for_lang を回す。
//! text/json で stdout 出力。auto は qname/root から推定。
//! `--daemon-port <port>` 指定時は LSP 直接 spawn を bypass、layer 2.5 daemon に投げる。

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::ckg::lsp::{find_closure_for_lang, ClosureNode, Direction};
use crate::handlers_find_symbol::{ensure_server_available, resolve_lang};
use crate::lsp_daemon::{ClosureNodePayload, DaemonClient};

/// `harness closure` CLI ハンドラ。
pub fn cmd_closure(
    qname: &str,
    depth: usize,
    direction: &str,
    root: Option<&str>,
    format: &str,
    lang_arg: &str,
    daemon_port: Option<u16>,
) -> Result<(), String> {
    let root_path = resolve_root(root)?;
    let dir = Direction::parse(direction)?;
    let nodes = if let Some(port) = daemon_port {
        fetch_closure_via_daemon(port, qname, depth, direction, &root_path)?
    } else {
        let lang = resolve_lang(lang_arg, qname, &root_path)?;
        ensure_server_available(lang)?;
        find_closure_for_lang(qname, depth, dir, lang, &root_path)?
    };
    match format {
        "json" => print_json(qname, depth, dir, &nodes)?,
        "text" => print_text(qname, depth, dir, &nodes),
        other => return Err(format!("unknown format: {other} (text|json)")),
    }
    Ok(())
}

fn fetch_closure_via_daemon(
    port: u16,
    qname: &str,
    depth: usize,
    direction: &str,
    root: &Path,
) -> Result<Vec<ClosureNode>, String> {
    let mut client = DaemonClient::connect(port)?;
    let payload = client.closure(qname, depth, direction, root, Duration::from_secs(120))?;
    Ok(payload.into_iter().map(closure_payload_to_node).collect())
}

fn closure_payload_to_node(p: ClosureNodePayload) -> ClosureNode {
    ClosureNode {
        name: p.name,
        file: p.file,
        line: p.line,
        depth: p.depth,
        direction: p.direction,
    }
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
