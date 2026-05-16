//! `harness closure <qname>` ハンドラ。
//!
//! `--lang auto|rust|ts|py|go` を受け、対応 LSP server を spawn して
//! find_closure_for_lang を回す。text/json で stdout 出力。
//! `--daemon-port <port>` / `--use-daemon` で layer 2.5 daemon 経由可。

use std::path::PathBuf;
use std::time::Duration;

use crate::ckg::lsp::{find_closure_for_lang, ClosureNode, Direction};
use crate::handlers_find_symbol::{ensure_server_available, open_client, resolve_lang};
use crate::lsp_daemon::ClosureNodePayload;

/// `harness closure` CLI ハンドラ。
#[allow(clippy::too_many_arguments)]
pub fn cmd_closure(
    qname: &str,
    depth: usize,
    direction: &str,
    root: Option<&str>,
    format: &str,
    lang_arg: &str,
    daemon_port: Option<u16>,
    use_daemon: bool,
) -> Result<(), String> {
    let root_path = resolve_root(root)?;
    let dir = Direction::parse(direction)?;
    let lang_lazy = || resolve_lang(lang_arg, qname, &root_path);
    let nodes = if let Some(mut c) = open_client(daemon_port, use_daemon, &root_path, &lang_lazy)? {
        let p = c.closure(qname, depth, direction, &root_path, Duration::from_secs(120))?;
        p.into_iter().map(closure_payload_to_node).collect()
    } else {
        let lang = lang_lazy()?;
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
