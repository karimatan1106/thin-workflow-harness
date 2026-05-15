//! `harness tested-by <qname>` ── 「qname をテストしている test 関数集合」抽出。
//!
//! `find_closure(direction=in)` の結果から test 関数のみフィルタする。
//! 判定順:
//!   (1) tree-sitter で `#[test]` 系 attr 直接検出（精度優先、対応 attr は
//!       `test_attrs.rs` 参照）
//!   (2) `#[cfg(test)] mod` 内側か（`test_mod_scan.rs`）── attr 無し helper も拾う
//!   (3) parse 不可 / file 不在のみ heuristic fallback
//! 同一 file の重複 parse を避けるため `find_tested_by` 内でローカルキャッシュ保持
//! （test 行 + cfg(test) mod range ペア）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::closure::{find_closure, ClosureNode, Direction, MAX_DEPTH};
use super::uri::percent_decode;
use crate::ckg::test_attrs::list_test_function_lines;
use crate::ckg::test_mod_scan::{line_in_ranges, list_cfg_test_mod_ranges};

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

/// attr ベース判定（line 単位）。判定順:
///   (1) attr 直接（`#[test]` 系）── line が test fn の開始行と一致
///   (2) `#[cfg(test)] mod` 内側 ── attr 無し helper も test 扱い
///   (3) parse 不可 / file 不在のみ heuristic に fallback
fn is_test_node_at(name: &str, file: &str, line: usize, cache: &mut TestLineCache) -> bool {
    match attr_entries(file, cache) {
        Some(entry) => {
            if entry.test_lines.contains(&line) {
                return true;
            }
            line_in_ranges(line, &entry.cfg_mod_ranges)
        }
        None => is_test_node(name, file),
    }
}

/// 1 file 分のキャッシュエントリ。test fn 開始行 + cfg(test) mod の line range ペア。
#[derive(Debug, Default, Clone)]
struct FileTestInfo {
    test_lines: Vec<usize>,
    cfg_mod_ranges: Vec<(usize, usize)>,
}

/// 同一 file の重複 parse を回避するキャッシュ。`Some(entry)` = parse 成功、
/// `None` = parse 不可 / file 不在 → heuristic fallback 対象。
type TestLineCache = HashMap<PathBuf, Option<FileTestInfo>>;

fn attr_entries<'a>(file: &str, cache: &'a mut TestLineCache) -> Option<&'a FileTestInfo> {
    // ClosureNode.file は uri_to_path_string で `file://` prefix だけ剥がれた
    // percent-encoded path（`c:/%E3%83%84...`）が来るので decode してから Path 化。
    let decoded = percent_decode(file);
    let path = PathBuf::from(&decoded);
    if !path.exists() {
        return None;
    }
    if !cache.contains_key(&path) {
        let parsed = match list_test_function_lines(&path).ok() {
            Some(lines) => {
                let ranges = list_cfg_test_mod_ranges(&path).unwrap_or_default();
                Some(FileTestInfo {
                    test_lines: lines,
                    cfg_mod_ranges: ranges,
                })
            }
            None => None,
        };
        cache.insert(path.clone(), parsed);
    }
    cache.get(&path).and_then(|x| x.as_ref())
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
    fn attr_entries_returns_none_for_missing_file() {
        let mut cache: TestLineCache = HashMap::new();
        assert!(attr_entries("/nonexistent/path/xxx.rs", &mut cache).is_none());
    }

    #[test]
    fn is_test_node_at_falls_back_to_heuristic_for_missing_file() {
        let mut cache: TestLineCache = HashMap::new();
        assert!(is_test_node_at("test_xxx", "/nonexistent/path/xxx.rs", 1, &mut cache));
        assert!(!is_test_node_at("helper", "/nonexistent/path/xxx.rs", 1, &mut cache));
    }

}
