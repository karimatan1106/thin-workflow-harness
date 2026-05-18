//! Python 専用 test 判定ヘルパ。`tested_lang.rs` から分離して 200 行ルールを保つ。
//!
//! - tree-sitter ベース attr 検出 (`test_attrs_py::list_test_function_lines`)
//! - path heuristic (`test_*.py` / `*_test.py` / `tests/`)
//! - name heuristic (`test_*` / `*_test` / `test`)
//!   の 3 段で `is_test_node_py` を判定する（Rust/TS と対称）。

use std::collections::HashMap;
use std::path::PathBuf;

use super::uri::percent_decode;
use crate::ckg::test_attrs_py::list_test_function_lines as list_test_function_lines_py;

/// Python 用の file 単位 test fn line キャッシュ。
pub type PyTestCache = HashMap<PathBuf, Option<Vec<usize>>>;

/// Python: tree-sitter attr → file path heuristic → name heuristic の 3 段。
pub fn is_test_node_py(name: &str, file: &str, line: usize, cache: &mut PyTestCache) -> bool {
    if let Some(lines) = py_attr_entries(file, cache) {
        if lines.contains(&line) {
            return true;
        }
    }
    is_test_file_py(file) || is_test_name_py(name)
}

fn py_attr_entries<'a>(file: &str, cache: &'a mut PyTestCache) -> Option<&'a Vec<usize>> {
    let decoded = percent_decode(file);
    let path = PathBuf::from(&decoded);
    if !path.exists() {
        return None;
    }
    let ext_ok = path.extension().and_then(|s| s.to_str()) == Some("py");
    if !ext_ok {
        return None;
    }
    if !cache.contains_key(&path) {
        let parsed = list_test_function_lines_py(&path).ok();
        cache.insert(path.clone(), parsed);
    }
    cache.get(&path).and_then(|x| x.as_ref())
}

/// pytest discovery のファイル名規約に従う path 判定。
pub fn is_test_file_py(file: &str) -> bool {
    let decoded = percent_decode(file);
    let norm = decoded.replace('\\', "/");
    let leaf = norm.rsplit('/').next().unwrap_or(&norm);
    if leaf.starts_with("test_") && leaf.ends_with(".py") {
        return true;
    }
    if leaf.ends_with("_test.py") {
        return true;
    }
    norm.starts_with("tests/") || norm.contains("/tests/")
}

/// 関数 / メソッド名から test 関数かどうかを判定。
pub fn is_test_name_py(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let leaf = match name.rsplit_once('.') {
        Some((_, leaf)) => leaf,
        None => name,
    };
    leaf.starts_with("test_") || leaf.ends_with("_test") || leaf == "test"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn py_test_file_path_heuristic() {
        assert!(is_test_file_py("tests/test_user.py"));
        assert!(is_test_file_py("src/test_user.py"));
        assert!(is_test_file_py("src/user_test.py"));
        assert!(is_test_file_py("tests/sub/foo.py"));
        assert!(is_test_file_py("pkg/tests/foo.py"));
        assert!(!is_test_file_py("src/user.py"));
        assert!(!is_test_file_py("src/testing.py"));
    }

    #[test]
    fn py_test_name_heuristic() {
        assert!(is_test_name_py("test_create_user"));
        assert!(is_test_name_py("create_user_test"));
        assert!(is_test_name_py("test"));
        assert!(is_test_name_py("TestUser.test_create"));
        assert!(!is_test_name_py("create_user"));
        assert!(!is_test_name_py("testing"));
        assert!(!is_test_name_py(""));
    }

    #[test]
    fn py_combined_node_with_cache() {
        let mut c: PyTestCache = HashMap::new();
        // file が test、name が非 test でも hit
        assert!(is_test_node_py("create_user", "tests/test_foo.py", 1, &mut c));
        // name が test、file が非 test でも hit
        assert!(is_test_node_py("test_create_user", "src/user.py", 1, &mut c));
        // どちらでもないと miss
        assert!(!is_test_node_py("create_user", "src/user.py", 1, &mut c));
    }
}
