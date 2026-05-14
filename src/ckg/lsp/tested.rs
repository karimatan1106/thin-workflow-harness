//! `harness tested-by <qname>` ── 「qname をテストしている test 関数集合」抽出。
//!
//! `find_closure(direction=in)` の結果から test 関数のみフィルタする。
//! 判定順: (1) tree-sitter で `#[test]` 系 attr 直接検出（精度優先、対応 attr は
//! `test_attrs.rs` 参照）、 (2) parse 不可 / file 不在のみ heuristic fallback。
//! 同一 file の重複 parse を避けるため `find_tested_by` 内でローカルキャッシュ保持。
//! `#[cfg(test)] mod` 親判定は次バッチ送り（attr 直接検出のみ）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::closure::{find_closure, ClosureNode, Direction, MAX_DEPTH};
use crate::ckg::test_attrs::list_test_function_lines;

/// tested-by 結果の 1 ノード。
#[derive(Debug, Clone, Serialize)]
pub struct TestedNode {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub depth: usize,
}

impl From<ClosureNode> for TestedNode {
    fn from(n: ClosureNode) -> Self {
        Self {
            name: n.name,
            file: n.file,
            line: n.line,
            depth: n.depth,
        }
    }
}

/// tested-by の既定 depth。
pub const DEFAULT_DEPTH: usize = 3;

/// test ノード判定（heuristic 単独、後方互換 API）。line 単位の attr 判定は
/// `find_tested_by` → `is_test_node_at` 経由。
pub fn is_test_node(name: &str, file: &str) -> bool {
    if is_test_file(file) {
        return true;
    }
    is_test_name(name)
}

/// attr ベース判定（line 単位）。parse 可能なら attr 結果のみ、parse 不可 /
/// file 不在のみ heuristic に fallback。
fn is_test_node_at(name: &str, file: &str, line: usize, cache: &mut TestLineCache) -> bool {
    match attr_lines(file, cache) {
        Some(lines) => lines.contains(&line),
        None => is_test_node(name, file),
    }
}

/// 同一 file の重複 parse を回避するキャッシュ。`Some(vec)` = parse 成功
/// (test fn の行番号)、`None` = parse 不可 → heuristic fallback 対象。
type TestLineCache = HashMap<PathBuf, Option<Vec<usize>>>;

fn attr_lines<'a>(file: &str, cache: &'a mut TestLineCache) -> Option<&'a Vec<usize>> {
    // ClosureNode.file は uri_to_path_string で `file://` prefix だけ剥がれた
    // percent-encoded path（`c:/%E3%83%84...`）が来るので decode してから Path 化。
    let decoded = percent_decode(file);
    let path = PathBuf::from(&decoded);
    if !path.exists() {
        return None;
    }
    if !cache.contains_key(&path) {
        let parsed = list_test_function_lines(&path).ok();
        cache.insert(path.clone(), parsed);
    }
    cache.get(&path).and_then(|x| x.as_ref())
}

/// 最小 percent-decode。`%HH` → 1 byte → UTF-8 lossy 復号。`%` を含まない
/// path には identity。rust-analyzer の non-ASCII URI を `Path` 化する前段。
fn percent_decode(s: &str) -> String {
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

/// `tests/` 配下 or `_test.rs` / `_tests.rs` 末尾。
fn is_test_file(file: &str) -> bool {
    let norm = file.replace('\\', "/");
    if norm.ends_with("_test.rs") || norm.ends_with("_tests.rs") {
        return true;
    }
    if norm.starts_with("tests/") || norm.contains("/tests/") {
        return true;
    }
    false
}

/// `test_` 接頭辞 or `_test` 接尾辞。
fn is_test_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let leaf = match name.rsplit_once("::") {
        Some((_, leaf)) => leaf,
        None => name,
    };
    leaf.starts_with("test_") || leaf.ends_with("_test")
}

/// `harness tested-by <qname>` 本体。`find_closure(In)` 結果から test 関数のみ抽出。
pub fn find_tested_by(
    server_cmd: &str,
    root: &Path,
    qname: &str,
    depth: usize,
    timeout: Duration,
) -> Result<Vec<TestedNode>, String> {
    let depth = depth.clamp(1, MAX_DEPTH);
    let nodes = find_closure(server_cmd, root, qname, depth, Direction::In, timeout)?;
    let mut cache: TestLineCache = HashMap::new();
    let filtered: Vec<TestedNode> = nodes
        .into_iter()
        .filter(|n| is_test_node_at(&n.name, &n.file, n.line, &mut cache))
        .map(TestedNode::from)
        .collect();
    Ok(filtered)
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn test_file_path_heuristic() {
        assert!(is_test_node("foo", "tests/it_user.rs"));
        assert!(is_test_node("foo", "crates/x/tests/it_user.rs"));
        assert!(is_test_node("foo", "src/user_test.rs"));
        assert!(is_test_node("foo", "src/user_tests.rs"));
        assert!(!is_test_node("foo", "src/user.rs"));
    }

    #[test]
    fn test_name_heuristic() {
        assert!(is_test_node("test_create_user", "src/lib.rs"));
        assert!(is_test_node("mod::sub::test_foo", "src/lib.rs"));
        assert!(is_test_node("my_test", "src/lib.rs"));
        assert!(!is_test_node("create_user", "src/lib.rs"));
        assert!(!is_test_node("", "src/lib.rs"));
    }

    #[test]
    fn attr_lines_returns_none_for_missing_file() {
        let mut cache: TestLineCache = HashMap::new();
        assert!(attr_lines("/nonexistent/path/xxx.rs", &mut cache).is_none());
    }

    #[test]
    fn is_test_node_at_falls_back_to_heuristic_for_missing_file() {
        let mut cache: TestLineCache = HashMap::new();
        assert!(is_test_node_at("test_xxx", "/nonexistent/path/xxx.rs", 1, &mut cache));
        assert!(!is_test_node_at("helper", "/nonexistent/path/xxx.rs", 1, &mut cache));
    }

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
