//! `harness tested-by <qname>` ── 「qname をテストしている test 関数集合」抽出。
//!
//! `find_closure(direction=in)` を呼んで、結果から test 関数のみフィルタする。
//! MVP heuristic (attr 解析しない、軽量):
//!   1. file path が `tests/` 配下、または `_test.rs` / `_tests.rs` で終わる
//!   2. または symbol name が `test_` で始まる、または `_test` で終わる
//!
//! いずれかが true なら test ノードとみなす。
//!
//! 既定 depth=3。tested-by 専用ビューを `TestedNode` で返す。
//! `#[test]` / `#[cfg(test)]` の attr ベース判定は tree-sitter outline 連携で
//! 後続バッチ送り（doc 参照: CKG layer 2 design memo）。

use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use super::closure::{find_closure, ClosureNode, Direction, MAX_DEPTH};

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

/// test ノード判定 heuristic（MVP、attr 解析なし）。
pub fn is_test_node(name: &str, file: &str) -> bool {
    if is_test_file(file) {
        return true;
    }
    is_test_name(name)
}

/// ファイルパスベース判定。`tests/` 配下 or `_test.rs` / `_tests.rs` で終わる。
fn is_test_file(file: &str) -> bool {
    // ファイルパス区切りは normalize（windows backslash 対応）。
    let norm = file.replace('\\', "/");
    if norm.ends_with("_test.rs") || norm.ends_with("_tests.rs") {
        return true;
    }
    // `tests/` セグメントを含むか（先頭でも中間でもよい）。
    if norm.starts_with("tests/") || norm.contains("/tests/") {
        return true;
    }
    false
}

/// 名前ベース判定。`test_` 接頭辞か `_test` 接尾辞。
fn is_test_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // qname 形式 (`mod::test_foo`) も拾えるよう、最後の `::` の後ろを見る。
    let leaf = match name.rsplit_once("::") {
        Some((_, leaf)) => leaf,
        None => name,
    };
    leaf.starts_with("test_") || leaf.ends_with("_test")
}

/// `harness tested-by <qname>` 本体。
///
/// `find_closure(direction=in)` の結果から test 関数のみフィルタする。
pub fn find_tested_by(
    server_cmd: &str,
    root: &Path,
    qname: &str,
    depth: usize,
    timeout: Duration,
) -> Result<Vec<TestedNode>, String> {
    let depth = depth.clamp(1, MAX_DEPTH);
    let nodes = find_closure(server_cmd, root, qname, depth, Direction::In, timeout)?;
    let filtered: Vec<TestedNode> = nodes
        .into_iter()
        .filter(|n| is_test_node(&n.name, &n.file))
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
}
