//! `find_tested_by` の Lang 版。`find_closure_for_lang(direction=in)` の結果から
//! `is_test_node_for_lang(node, lang)` で test 関数のみフィルタする。
//!
//! - Rust: attr ベース (`test_attrs::list_test_function_lines`) +
//!   `cfg(test) mod` 親階層判定 (`test_mod_scan`) + heuristic fallback
//! - TS: `tested_ts::is_test_node_ts` (tree-sitter + heuristic fallback)
//! - Py: `tested_py::is_test_node_py` (tree-sitter + heuristic fallback)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::closure::{ClosureNode, Direction, MAX_DEPTH};
use super::closure_lang::find_closure_for_lang;
use super::lang::Lang;
use super::tested::TestedNode;
use super::tested_py::{is_test_node_py, PyTestCache};
use super::tested_ts::{is_test_node_ts, TsTestCache};
use super::uri::percent_decode;
use crate::ckg::test_attrs::list_test_function_lines;
use crate::ckg::test_mod_scan::{line_in_ranges, list_cfg_test_mod_ranges};

/// `harness tested-by <qname> --lang ...` 本体。
pub fn find_tested_by_for_lang(
    qname: &str,
    depth: usize,
    lang: Lang,
    root: &Path,
) -> Result<Vec<TestedNode>, String> {
    let depth = depth.clamp(1, MAX_DEPTH);
    let nodes = find_closure_for_lang(qname, depth, Direction::In, lang, root)?;
    let mut caches = TestCaches::default();
    let filtered: Vec<TestedNode> = nodes
        .into_iter()
        .filter(|n| is_test_node_for_lang(n, lang, &mut caches))
        .map(TestedNode::from)
        .collect();
    Ok(filtered)
}

/// Lang ごとの test fn キャッシュをまとめた struct。
#[derive(Default)]
struct TestCaches {
    rust: RustTestCache,
    ts: TsTestCache,
    py: PyTestCache,
}

/// 1 file 分の Rust attr 情報キャッシュ。
#[derive(Debug, Default, Clone)]
struct RustTestInfo {
    test_lines: Vec<usize>,
    cfg_mod_ranges: Vec<(usize, usize)>,
}

/// Rust 用の file 単位 attr キャッシュ。
type RustTestCache = HashMap<PathBuf, Option<RustTestInfo>>;

/// Lang ごとの test ノード判定。
fn is_test_node_for_lang(node: &ClosureNode, lang: Lang, caches: &mut TestCaches) -> bool {
    match lang {
        Lang::Rust => is_test_node_rust(&node.name, &node.file, node.line, &mut caches.rust),
        Lang::Ts => is_test_node_ts(&node.name, &node.file, node.line, &mut caches.ts),
        Lang::Py => is_test_node_py(&node.name, &node.file, node.line, &mut caches.py),
    }
}

/// Rust: attr → cfg(test) mod 内側 → heuristic fallback の 3 段。
fn is_test_node_rust(name: &str, file: &str, line: usize, cache: &mut RustTestCache) -> bool {
    match rust_attr_entries(file, cache) {
        Some(entry) => {
            if entry.test_lines.contains(&line) {
                return true;
            }
            line_in_ranges(line, &entry.cfg_mod_ranges)
        }
        None => is_test_file_rust(file) || is_test_name_rust(name),
    }
}

fn rust_attr_entries<'a>(
    file: &str,
    cache: &'a mut RustTestCache,
) -> Option<&'a RustTestInfo> {
    let decoded = percent_decode(file);
    let path = PathBuf::from(&decoded);
    if !path.exists() {
        return None;
    }
    if !cache.contains_key(&path) {
        let parsed = match list_test_function_lines(&path).ok() {
            Some(lines) => {
                let ranges = list_cfg_test_mod_ranges(&path).unwrap_or_default();
                Some(RustTestInfo { test_lines: lines, cfg_mod_ranges: ranges })
            }
            None => None,
        };
        cache.insert(path.clone(), parsed);
    }
    cache.get(&path).and_then(|x| x.as_ref())
}

fn is_test_file_rust(file: &str) -> bool {
    let norm = file.replace('\\', "/");
    if norm.ends_with("_test.rs") || norm.ends_with("_tests.rs") {
        return true;
    }
    norm.starts_with("tests/") || norm.contains("/tests/")
}

fn is_test_name_rust(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let leaf = match name.rsplit_once("::") {
        Some((_, leaf)) => leaf,
        None => name,
    };
    leaf.starts_with("test_") || leaf.ends_with("_test")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn mk(name: &str, file: &str) -> ClosureNode {
        ClosureNode {
            name: name.to_string(),
            file: file.to_string(),
            line: 1,
            depth: 1,
            direction: "in".to_string(),
        }
    }

    fn mk_at(name: &str, file: &str, line: usize) -> ClosureNode {
        ClosureNode {
            name: name.to_string(),
            file: file.to_string(),
            line,
            depth: 1,
            direction: "in".to_string(),
        }
    }

    #[test]
    fn ts_test_file_path_heuristic() {
        let mut caches = TestCaches::default();
        assert!(is_test_node_for_lang(&mk("foo", "src/user.test.ts"), Lang::Ts, &mut caches));
        assert!(is_test_node_for_lang(&mk("foo", "src/user.spec.ts"), Lang::Ts, &mut caches));
        assert!(is_test_node_for_lang(&mk("foo", "src/__tests__/user.ts"), Lang::Ts, &mut caches));
        assert!(is_test_node_for_lang(&mk("foo", "tests/user.ts"), Lang::Ts, &mut caches));
        assert!(!is_test_node_for_lang(&mk("foo", "src/user.ts"), Lang::Ts, &mut caches));
    }

    #[test]
    fn ts_test_name_heuristic() {
        let mut caches = TestCaches::default();
        assert!(is_test_node_for_lang(&mk("describe", "src/user.ts"), Lang::Ts, &mut caches));
        assert!(is_test_node_for_lang(&mk("it", "src/user.ts"), Lang::Ts, &mut caches));
        assert!(is_test_node_for_lang(&mk("test_foo", "src/user.ts"), Lang::Ts, &mut caches));
        assert!(is_test_node_for_lang(&mk("my_test", "src/user.ts"), Lang::Ts, &mut caches));
        assert!(!is_test_node_for_lang(&mk("createUser", "src/user.ts"), Lang::Ts, &mut caches));
    }

    #[test]
    fn ts_test_attr_detection_via_tree_sitter() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("sample.ts");
        let src = "\ndescribe(\"user\", () => {\n  it(\"foo\", () => { expect(1).toBe(1); });\n});\n";
        fs::write(&path, src).expect("write");
        let file_str = path.to_string_lossy().to_string();

        let mut caches = TestCaches::default();
        let node = mk_at("createUser", &file_str, 2);
        assert!(
            is_test_node_for_lang(&node, Lang::Ts, &mut caches),
            "tree-sitter describe block 内 line 2 が hit すべき"
        );

        let node_outside = mk_at("createUser", &file_str, 100);
        assert!(
            !is_test_node_for_lang(&node_outside, Lang::Ts, &mut caches),
            "line 100 は範囲外で hit してはならない"
        );
    }
}
