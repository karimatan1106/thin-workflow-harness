//! daemon client -- line-based JSON over TCP.
//!
//! - `connect(port)` opens 127.0.0.1:port
//! - `connect_or_spawn(lang, root, timeout)` で既存 daemon 接続 or auto-spawn
//! - `send_request_recv_response` writes 1 JSON line + reads 1 JSON line
//! - per-op convenience methods live in `client_ops.rs`
//! - request id auto-assigned via atomic counter
//! - PoC: 1 connection processes 1 request at a time (ordering preserved)
//! - NOTE: 真の detach (Unix setsid / Windows DETACHED_PROCESS) は次バッチ送り。
//!   現状は Stdio::null() で stdin/stdout/stderr を切るのみ。親が die すると
//!   子の生存は OS 依存。

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::ckg::lsp::Lang;

use super::port_file;
use super::protocol::{Op, Request, Response};

/// Connected daemon client. 1 instance = 1 TCP connection.
pub struct DaemonClient {
    pub(crate) reader: BufReader<TcpStream>,
    pub(crate) writer: TcpStream,
    pub(crate) next_id: AtomicU64,
}

impl DaemonClient {
    /// Connect to 127.0.0.1:port. Errors immediately if daemon is not listening.
    pub fn connect(port: u16) -> Result<Self, String> {
        let stream = TcpStream::connect(("127.0.0.1", port))
            .map_err(|e| format!("connect 127.0.0.1:{port}: {e}"))?;
        let read = stream.try_clone().map_err(|e| format!("clone: {e}"))?;
        Ok(DaemonClient {
            reader: BufReader::new(read),
            writer: stream,
            next_id: AtomicU64::new(1),
        })
    }

    /// 既存 daemon に接続するか、無ければ auto-spawn して接続する。
    ///
    /// 1. port file あり → port 読み出し → connect 試行 (OK ならそのまま返す)
    /// 2. NG (stale) → port file 削除
    /// 3. self exe を `lsp-daemon serve --lang <lang> --root <root> --port 0` で spawn
    /// 4. port file 出現を `spawn_timeout` まで 200ms 間隔で poll
    /// 5. file 出現 → port 読み出し → connect
    pub fn connect_or_spawn(
        lang: Lang,
        root: &Path,
        spawn_timeout: Duration,
    ) -> Result<Self, String> {
        let lang_s = lang_to_str(lang);
        let pf_path = port_file::port_file_path(lang_s, root)?;

        if let Ok(content) = port_file::read(&pf_path) {
            if let Ok(client) = Self::connect(content.port) {
                return Ok(client);
            }
            let _ = port_file::delete(&pf_path);
        }

        let self_exe = std::env::current_exe()
            .map_err(|e| format!("current_exe: {e}"))?;
        let _child = std::process::Command::new(&self_exe)
            .arg("lsp-daemon")
            .arg("serve")
            .arg("--lang")
            .arg(lang_s)
            .arg("--root")
            .arg(root)
            .arg("--port")
            .arg("0")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn daemon: {e}"))?;

        let deadline = Instant::now() + spawn_timeout;
        let poll_interval = Duration::from_millis(200);
        while Instant::now() < deadline {
            std::thread::sleep(poll_interval);
            if let Ok(content) = port_file::read(&pf_path) {
                if let Ok(client) = Self::connect(content.port) {
                    return Ok(client);
                }
            }
        }
        Err(format!("daemon spawn timed out ({:?})", spawn_timeout))
    }

    /// Optional: set TCP read/write timeout.
    pub fn set_timeout(&mut self, dur: Duration) -> Result<(), String> {
        self.writer
            .set_read_timeout(Some(dur))
            .map_err(|e| format!("set_read_timeout: {e}"))?;
        self.writer
            .set_write_timeout(Some(dur))
            .map_err(|e| format!("set_write_timeout: {e}"))?;
        Ok(())
    }

    /// Allocate the next request id (monotonic).
    pub(crate) fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a Request and read back the matching Response. Validates id.
    pub(crate) fn send_request_recv_response(&mut self, op: Op) -> Result<Response, String> {
        let id = self.next_id();
        let req = Request { id, op };
        let body = serde_json::to_string(&req).map_err(|e| format!("serialize: {e}"))?;
        self.writer
            .write_all(body.as_bytes())
            .map_err(|e| format!("write: {e}"))?;
        self.writer
            .write_all(b"\n")
            .map_err(|e| format!("write nl: {e}"))?;
        self.writer.flush().map_err(|e| format!("flush: {e}"))?;

        let mut line = String::new();
        let n = self
            .reader
            .read_line(&mut line)
            .map_err(|e| format!("read_line: {e}"))?;
        if n == 0 {
            return Err("daemon closed connection".to_string());
        }
        let resp: Response = serde_json::from_str(line.trim())
            .map_err(|e| format!("parse resp: {e}"))?;
        if resp.id != id {
            return Err(format!("id mismatch: expected {id}, got {}", resp.id));
        }
        Ok(resp)
    }
}

pub(crate) fn lang_to_str(lang: Lang) -> &'static str {
    match lang {
        Lang::Rust => "rust",
        Lang::Ts => "ts",
        Lang::Py => "py",
        Lang::Go => "go",
    }
}
