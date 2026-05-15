//! Rust 専用 test 判定ヘルパ。`tested_lang.rs` から分離して 200 行ルールを保つ。
//!
//! - tree-sitter ベース attr 検出 (`test_attrs::list_test_function_lines`)
//! - `#[cfg(test)] mod` 親階層 range 判定 (`test_mod_scan`)
//! - path heuristic (`*_test.rs` / `tests/`)
//! - name heuristic (`test_*` / `*_test`)
//!   の 3 段で `is_test_node_rust` を判定する（TS/Py/Go と対称）。

use std::collections::HashMap;
use std::path::PathBuf;

use super::uri::percent_decode;
use crate::ckg::test_attrs::list_test_function_lines;
use crate::ckg::test_mod_scan::{line_in_ranges, list_cfg_test_mod_ranges};

/// 1 file 分の Rust attr 情報キャッシュエントリ。
#[derive(Debug, Default, Clone)]
pub struct RustTestInfo {
    pub test_lines: Vec<usize>,
    pub cfg_mod_ranges: Vec<(usize, usize)>,
}

/// Rust 用の file 単位 attr キャッシュ。
pub type RustTestCache = HashMap<PathBuf, Option<RustTestInfo>>;

/// Rust: attr → cfg(test) mod 内側 → heuristic fallback の 3 段。
pub fn is_test_node_rust(
    name: &str,
    file: &str,
    line: usize,
    cache: &mut RustTestCache,
) -> bool {
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

pub fn is_test_file_rust(file: &str) -> bool {
    let norm = file.replace('\\', "/");
    if norm.ends_with("_test.rs") || norm.ends_with("_tests.rs") {
        return true;
    }
    norm.starts_with("tests/") || norm.contains("/tests/")
}

pub fn is_test_name_rust(name: &str) -> bool {
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

    #[test]
    fn rust_test_file_heuristic() {
        assert!(is_test_file_rust("src/foo_test.rs"));
        assert!(is_test_file_rust("src/foo_tests.rs"));
        assert!(is_test_file_rust("tests/foo.rs"));
        assert!(is_test_file_rust("crates/x/tests/foo.rs"));
        assert!(!is_test_file_rust("src/foo.rs"));
    }

    #[test]
    fn rust_test_name_heuristic() {
        assert!(is_test_name_rust("test_foo"));
        assert!(is_test_name_rust("foo_test"));
        assert!(is_test_name_rust("mod::test_foo"));
        assert!(!is_test_name_rust("create_user"));
        assert!(!is_test_name_rust(""));
    }
}
