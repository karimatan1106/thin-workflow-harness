//! foreground LSP daemon -- TCP localhost + line-based JSON.
//!
//! run_daemon(lang, root, port) is blocking. Ctrl-C / kill terminates.
//! (graceful shutdown is next batch.)
//!
//! - one-time start_and_warm_up(lang, root) spawns LspClient
//! - Arc<Mutex<LspClient>> shares the client across connections
//! - each connection: BufReader::read_line -> 1 line = 1 request

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::ckg::lsp::{
    find_symbol_for_lang_with_client, start_and_warm_up, Lang, LspClient,
};

use super::protocol::{Op, Request, Response, SymbolPayload};

/// Start the foreground daemon. port=0 means OS-assigned.
pub fn run_daemon(lang: Lang, root: PathBuf, port: u16) -> Result<(), String> {
    let client = start_and_warm_up(lang, &root)
        .map_err(|e| format!("warm-up failed: {e}"))?;
    let client = Arc::new(Mutex::new(client));

    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("bind 127.0.0.1:{port}: {e}"))?;
    let bound = listener
        .local_addr()
        .map_err(|e| format!("local_addr: {e}"))?;
    eprintln!(
        "[daemon] lang={} root={} port={}",
        lang_str(lang),
        root.display(),
        bound.port()
    );

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if let Err(e) = handle_connection(&client, lang, &root, s) {
                    eprintln!("[daemon] connection error: {e}");
                }
            }
            Err(e) => {
                eprintln!("[daemon] accept error: {e}");
            }
        }
    }
    Ok(())
}

fn handle_connection(
    client: &Arc<Mutex<LspClient>>,
    lang: Lang,
    root: &Path,
    stream: TcpStream,
) -> Result<(), String> {
    let read_stream = stream
        .try_clone()
        .map_err(|e| format!("clone stream: {e}"))?;
    let mut reader = BufReader::new(read_stream);
    let mut writer = stream;
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| format!("read_line: {e}"))?;
        if n == 0 {
            return Ok(());
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let resp = match serde_json::from_str::<Request>(trimmed) {
            Ok(req) => handle_request(client, lang, root, req),
            Err(e) => Response {
                id: 0,
                ok: false,
                data: Value::Null,
                error: Some(format!("parse request: {e}")),
            },
        };
        let body = serde_json::to_string(&resp)
            .map_err(|e| format!("serialize resp: {e}"))?;
        writer
            .write_all(body.as_bytes())
            .map_err(|e| format!("write: {e}"))?;
        writer
            .write_all(b"\n")
            .map_err(|e| format!("write nl: {e}"))?;
        writer.flush().map_err(|e| format!("flush: {e}"))?;
    }
}

fn handle_request(
    client: &Arc<Mutex<LspClient>>,
    lang: Lang,
    _root: &Path,
    req: Request,
) -> Response {
    let id = req.id;
    match req.op {
        Op::FindSymbol(p) => {
            let root_path = PathBuf::from(&p.root);
            let timeout = Duration::from_millis(p.timeout_ms);
            let mut guard = match client.lock() {
                Ok(g) => g,
                Err(poison) => poison.into_inner(),
            };
            let res = find_symbol_for_lang_with_client(
                &mut guard,
                lang,
                &root_path,
                &p.qname,
                p.kind.as_deref(),
                timeout,
            );
            match res {
                Ok(syms) => {
                    let payload: Vec<SymbolPayload> =
                        syms.into_iter().map(SymbolPayload::from).collect();
                    Response {
                        id,
                        ok: true,
                        data: serde_json::to_value(&payload).unwrap_or(Value::Null),
                        error: None,
                    }
                }
                Err(e) => Response {
                    id,
                    ok: false,
                    data: Value::Null,
                    error: Some(e),
                },
            }
        }
    }
}

fn lang_str(l: Lang) -> &'static str {
    match l {
        Lang::Rust => "rust",
        Lang::Ts => "ts",
        Lang::Py => "py",
        Lang::Go => "go",
    }
}
