//! JSON-RPC over LSP wire framing。
//!
//! ヘッダは `Content-Length: N\r\n\r\n` 固定 ── `Content-Type` は LSP では省略可。
//! ボディは UTF-8 の JSON。BufReader 経由で同期 read/write。

use std::io::{BufRead, Write};

/// 1メッセージ書き出し（header + body）。
pub fn write_message<W: Write>(w: &mut W, body: &str) -> Result<(), String> {
    let bytes = body.as_bytes();
    write!(w, "Content-Length: {}\r\n\r\n", bytes.len())
        .map_err(|e| format!("write header: {e}"))?;
    w.write_all(bytes).map_err(|e| format!("write body: {e}"))?;
    w.flush().map_err(|e| format!("flush: {e}"))?;
    Ok(())
}

/// 1メッセージ読み込み（header parse → body N バイト読む）。
/// EOF（ヘッダ 0 行）は `Ok(None)`。
pub fn read_message<R: BufRead>(r: &mut R) -> Result<Option<String>, String> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = r
            .read_line(&mut line)
            .map_err(|e| format!("read header: {e}"))?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed
            .strip_prefix("Content-Length:")
            .or_else(|| trimmed.strip_prefix("content-length:"))
        {
            let n: usize = rest
                .trim()
                .parse()
                .map_err(|e| format!("bad Content-Length '{rest}': {e}"))?;
            content_length = Some(n);
        }
        // 他のヘッダ（Content-Type 等）は無視。
    }
    let len = content_length.ok_or_else(|| "missing Content-Length".to_string())?;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)
        .map_err(|e| format!("read body ({len}B): {e}"))?;
    let s = String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))?;
    Ok(Some(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn roundtrip_one_message() {
        let mut out: Vec<u8> = Vec::new();
        write_message(&mut out, "{\"x\":1}").expect("write ok");
        let mut cur = Cursor::new(out);
        let body = read_message(&mut cur).expect("read ok").expect("some");
        assert_eq!(body, "{\"x\":1}");
    }

    #[test]
    fn eof_returns_none() {
        let mut cur = Cursor::new(Vec::<u8>::new());
        let r = read_message(&mut cur).expect("ok");
        assert!(r.is_none());
    }

    #[test]
    fn ignores_extra_headers() {
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"Content-Length: 3\r\n");
        out.extend_from_slice(b"Content-Type: application/vscode-jsonrpc; charset=utf-8\r\n");
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(b"abc");
        let mut cur = Cursor::new(out);
        let body = read_message(&mut cur).expect("ok").expect("some");
        assert_eq!(body, "abc");
    }
}
