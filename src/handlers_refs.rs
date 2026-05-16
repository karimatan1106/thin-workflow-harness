//! `harness refs <qname>` / `harness callers <qname>` ハンドラ。
//!
//! LSP server を spawn して位置解決 → references / callHierarchy を 1 往復させ、
//! text/json で stdout に出力する。`--lang auto|rust|ts|py|go` で言語を選択。
//! `--daemon-port <port>` 指定時は LSP 直接 spawn を bypass、layer 2.5 daemon に投げる。

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::ckg::lsp::{
    find_callers_for_lang, find_refs_for_lang, CallerInfo, RefLocation,
};
use crate::handlers_find_symbol::{ensure_server_available, resolve_lang};
use crate::lsp_daemon::{CallerPayload, DaemonClient, RefPayload};

/// `harness refs` CLI ハンドラ。
pub fn cmd_refs(
    qname: &str,
    root: Option<&str>,
    format: &str,
    lang_arg: &str,
    daemon_port: Option<u16>,
) -> Result<(), String> {
    let root_path = resolve_root(root)?;
    let refs = if let Some(port) = daemon_port {
        fetch_refs_via_daemon(port, qname, &root_path)?
    } else {
        let lang = resolve_lang(lang_arg, qname, &root_path)?;
        ensure_server_available(lang)?;
        find_refs_for_lang(lang, &root_path, qname, Duration::from_secs(30))?
    };
    match format {
        "json" => print_refs_json(qname, &refs)?,
        "text" => print_refs_text(qname, &refs),
        other => return Err(format!("unknown format: {other} (text|json)")),
    }
    Ok(())
}

/// `harness callers` CLI ハンドラ。
pub fn cmd_callers(
    qname: &str,
    root: Option<&str>,
    format: &str,
    lang_arg: &str,
    daemon_port: Option<u16>,
) -> Result<(), String> {
    let root_path = resolve_root(root)?;
    let callers = if let Some(port) = daemon_port {
        fetch_callers_via_daemon(port, qname, &root_path)?
    } else {
        let lang = resolve_lang(lang_arg, qname, &root_path)?;
        ensure_server_available(lang)?;
        find_callers_for_lang(lang, &root_path, qname, Duration::from_secs(30))?
    };
    match format {
        "json" => print_callers_json(qname, &callers)?,
        "text" => print_callers_text(qname, &callers),
        other => return Err(format!("unknown format: {other} (text|json)")),
    }
    Ok(())
}

fn fetch_refs_via_daemon(
    port: u16,
    qname: &str,
    root: &Path,
) -> Result<Vec<RefLocation>, String> {
    let mut client = DaemonClient::connect(port)?;
    let payload = client.refs(qname, root, Duration::from_secs(60))?;
    Ok(payload.into_iter().map(ref_payload_to_loc).collect())
}

fn fetch_callers_via_daemon(
    port: u16,
    qname: &str,
    root: &Path,
) -> Result<Vec<CallerInfo>, String> {
    let mut client = DaemonClient::connect(port)?;
    let payload = client.callers(qname, root, Duration::from_secs(60))?;
    Ok(payload.into_iter().map(caller_payload_to_info).collect())
}

fn ref_payload_to_loc(p: RefPayload) -> RefLocation {
    RefLocation { file: p.file, line: p.line, col: p.col }
}

fn caller_payload_to_info(p: CallerPayload) -> CallerInfo {
    CallerInfo { name: p.name, file: p.file, line: p.line }
}

fn resolve_root(root: Option<&str>) -> Result<PathBuf, String> {
    match root {
        Some(r) => Ok(PathBuf::from(r)),
        None => std::env::current_dir().map_err(|e| format!("cwd: {e}")),
    }
}

fn print_refs_text(qname: &str, refs: &[RefLocation]) {
    println!("references to `{qname}`:");
    for r in refs {
        println!("  {}:{}:{}", r.file, r.line, r.col);
    }
}

fn print_refs_json(qname: &str, refs: &[RefLocation]) -> Result<(), String> {
    let payload = serde_json::json!({ "qname": qname, "references": refs });
    let s = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("serialize: {e}"))?;
    println!("{s}");
    Ok(())
}

fn print_callers_text(qname: &str, callers: &[CallerInfo]) {
    println!("callers of `{qname}`:");
    for c in callers {
        println!("  {} at {}:{}", c.name, c.file, c.line);
    }
}

fn print_callers_json(qname: &str, callers: &[CallerInfo]) -> Result<(), String> {
    let payload = serde_json::json!({ "qname": qname, "callers": callers });
    let s = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("serialize: {e}"))?;
    println!("{s}");
    Ok(())
}
