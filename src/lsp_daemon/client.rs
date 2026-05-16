//! daemon client -- line-based JSON over TCP.
//!
//! - connect(port) opens 127.0.0.1:port
//! - find_symbol(qname, root, kind, timeout) does 1 round-trip
//! - request id auto-assigned via atomic counter
//! - PoC: 1 connection processes 1 request at a time (ordering preserved)

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::Value;

use super::protocol::{FindSymbolParams, Op, Request, Response, SymbolPayload};

/// Connected daemon client. 1 instance = 1 TCP connection.
pub struct DaemonClient {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    next_id: AtomicU64,
}

impl DaemonClient {
    /// Connect to 127.0.0.1:port. Errors immediately if daemon is not listening.
    pub fn connect(port: u16) -> Result<Self, String> {
        let stream = TcpStream::connect(("127.0.0.1", port))
            .map_err(|e| format!("connect 127.0.0.1:{port}: {e}"))?;
        let read = stream
            .try_clone()
            .map_err(|e| format!("clone: {e}"))?;
        Ok(DaemonClient {
            reader: BufReader::new(read),
            writer: stream,
            next_id: AtomicU64::new(1),
        })
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

    /// Send find_symbol to daemon.
    pub fn find_symbol(
        &mut self,
        qname: &str,
        root: &Path,
        kind: Option<&str>,
        timeout: Duration,
    ) -> Result<Vec<SymbolPayload>, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = Request {
            id,
            op: Op::FindSymbol(FindSymbolParams {
                qname: qname.to_string(),
                root: root.to_string_lossy().to_string(),
                kind: kind.map(|s| s.to_string()),
                timeout_ms: timeout.as_millis() as u64,
            }),
        };
        let body = serde_json::to_string(&req)
            .map_err(|e| format!("serialize: {e}"))?;
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
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "unknown error".to_string()));
        }
        let payload: Vec<SymbolPayload> = match resp.data {
            Value::Null => Vec::new(),
            other => serde_json::from_value(other)
                .map_err(|e| format!("decode payload: {e}"))?,
        };
        Ok(payload)
    }
}
