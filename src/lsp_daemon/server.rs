//! foreground LSP daemon -- TCP localhost + line-based JSON.
//!
//! - one-time start_and_warm_up(lang, root) spawns the LspClient
//! - Arc<Mutex<LspClient>> shares the client across connections
//! - each connection: BufReader::read_line -> 1 line = 1 request
//! - per-request dispatch is delegated to `dispatch::handle_request`
//! - listener bind 後の実 port を port_file に書き出し、Drop で削除する。

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::ckg::lsp::{start_and_warm_up, Lang, LspClient};

use super::dispatch::handle_request;
use super::port_file::{self, PortFileContent};
use super::protocol::{Request, Response};
use super::state::DaemonState;

/// daemon shutdown 時に port file を best-effort 削除する RAII guard。
struct PortFileGuard {
    path: PathBuf,
}

impl Drop for PortFileGuard {
    fn drop(&mut self) {
        let _ = port_file::delete(&self.path);
    }
}

/// Start the foreground daemon. port=0 means OS-assigned.
pub fn run_daemon(lang: Lang, root: PathBuf, port: u16) -> Result<(), String> {
    let client =
        start_and_warm_up(lang, &root).map_err(|e| format!("warm-up failed: {e}"))?;
    let client = Arc::new(Mutex::new(client));
    let state = Arc::new(DaemonState::new(lang));

    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("bind 127.0.0.1:{port}: {e}"))?;
    let bound = listener
        .local_addr()
        .map_err(|e| format!("local_addr: {e}"))?;
    let lang_s = lang_str(lang);
    eprintln!(
        "[daemon] lang={} root={} port={}",
        lang_s,
        root.display(),
        bound.port()
    );

    // port file 書き出し (best-effort、失敗は warn だけ)
    let pf_path = port_file::port_file_path(lang_s, &root)?;
    let content = PortFileContent {
        pid: std::process::id(),
        port: bound.port(),
        started_at_ms: port_file::now_ms(),
    };
    if let Err(e) = port_file::write(&pf_path, &content) {
        eprintln!("[daemon] port_file write failed: {e}");
    } else {
        eprintln!("[daemon] port_file={}", pf_path.display());
    }
    let _guard = PortFileGuard { path: pf_path };

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if let Err(e) = handle_connection(&client, &state, lang, &root, s) {
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
    state: &Arc<DaemonState>,
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
            Ok(req) => handle_request(client, state, lang, root, req),
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

fn lang_str(l: Lang) -> &'static str {
    match l {
        Lang::Rust => "rust",
        Lang::Ts => "ts",
        Lang::Py => "py",
        Lang::Go => "go",
    }
}
