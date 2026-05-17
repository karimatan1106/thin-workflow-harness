//! daemon client -- line-based JSON over TCP.
//!
//! - `connect(port)` opens 127.0.0.1:port
//! - `connect_or_spawn(lang, root, timeout)` で既存 daemon 接続 or auto-spawn
//! - `send_request_recv_response` writes 1 JSON line + reads 1 JSON line
//! - per-op convenience methods live in `client_ops.rs`
//! - request id auto-assigned via atomic counter
//! - PoC: 1 connection processes 1 request at a time (ordering preserved)
//! - Windows: DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP で子を完全 detach。
//!   msys2 pipeline で head した時の子孫 handle inherit ブロックが解消。

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::os::windows::process::CommandExt;
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
        let mut cmd = std::process::Command::new(&self_exe);
        let idle_min: u64 = std::env::var("HARNESS_DAEMON_IDLE_MIN")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);
        let idle_min_s = idle_min.to_string();
        cmd.arg("lsp-daemon")
            .arg("serve")
            .arg("--lang")
            .arg(lang_s)
            .arg("--root")
            .arg(root)
            .arg("--port")
            .arg("0")
            .arg("--idle-timeout-min")
            .arg(&idle_min_s)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        // DETACHED_PROCESS = 0x00000008, CREATE_NEW_PROCESS_GROUP = 0x00000200
        // 親プロセスから完全に切り離す。msys2 pipeline の head ブロックも解消。
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
        let _child = cmd.spawn().map_err(|e| format!("spawn daemon: {e}"))?;

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
            .write_all(b"
")
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
