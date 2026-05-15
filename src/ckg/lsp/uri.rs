//! LSP `file://` URI 用の最小 percent-decode。
//!
//! rust-analyzer は workspace 内 file URI を返すが、non-ASCII path は
//! `c:/%E3%83%84...` のように percent-encoded で来る。`PathBuf` 化前に decode する。

/// `%HH` → 1 byte → UTF-8 lossy 復号。`%` を含まない path には identity。
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn percent_decode_handles_utf8_and_passthrough() {
        // %E3%83%84 = "ツ" (UTF-8 3byte)
        assert_eq!(percent_decode("c:/%E3%83%84/x.rs"), "c:/ツ/x.rs");
        // 含まない path は identity
        assert_eq!(percent_decode("/home/user/x.rs"), "/home/user/x.rs");
        // 末尾の incomplete escape は素通し
        assert_eq!(percent_decode("abc%"), "abc%");
        assert_eq!(percent_decode("abc%E"), "abc%E");
        // 大文字小文字どちらも OK
        assert_eq!(percent_decode("%e3%83%84"), "ツ");
        // 非 hex の %XX は素通し
        assert_eq!(percent_decode("a%ZZb"), "a%ZZb");
    }
}
