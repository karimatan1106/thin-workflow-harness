//! Go 専用 test 判定ヘルパ。`tested_lang.rs` から分離して 200 行ルールを保つ。
//!
//! - tree-sitter ベース attr 検出 (`test_attrs_go::list_test_function_lines`)
//! - path heuristic (`*_test.go`)
//! - name heuristic (`Test*` / `Benchmark*` / `Example*` / `Fuzz*`)
//!   の 3 段で `is_test_node_go` を判定する（Rust/TS/Py と対称）。

use std::collections::HashMap;
use std::path::PathBuf;

use super::uri::percent_decode;
use crate::ckg::test_attrs_go::list_test_function_lines as list_test_function_lines_go;

/// Go 用の file 単位 test fn line キャッシュ。
pub type GoTestCache = HashMap<PathBuf, Option<Vec<usize>>>;

/// Go: tree-sitter attr → path heuristic → name heuristic の 3 段。
pub fn is_test_node_go(name: &str, file: &str, line: usize, cache: &mut GoTestCache) -> bool {
    if let Some(lines) = go_attr_entries(file, cache) {
        if lines.contains(&line) {
            return true;
        }
    }
    is_test_file_go(file) || is_test_name_go(name)
}

fn go_attr_entries<'a>(file: &str, cache: &'a mut GoTestCache) -> Option<&'a Vec<usize>> {
    let decoded = percent_decode(file);
    let path = PathBuf::from(&decoded);
    if !path.exists() {
        return None;
    }
    let ext_ok = path.extension().and_then(|s| s.to_str()) == Some("go");
    if !ext_ok {
        return None;
    }
    if !cache.contains_key(&path) {
        let parsed = list_test_function_lines_go(&path).ok();
        cache.insert(path.clone(), parsed);
    }
    cache.get(&path).and_then(|x| x.as_ref())
}

/// Go test 規約（`*_test.go`）。
pub fn is_test_file_go(file: &str) -> bool {
    let decoded = percent_decode(file);
    let norm = decoded.replace('\\', "/");
    let leaf = norm.rsplit('/').next().unwrap_or(&norm);
    leaf.ends_with("_test.go")
}

/// Go test 関数名の prefix 規約。
/// - `TestXxx(t *testing.T)`           ── 通常 test
/// - `BenchmarkXxx(b *testing.B)`      ── benchmark
/// - `ExampleXxx()`                    ── godoc example
/// - `FuzzXxx(f *testing.F)`           ── fuzz test (Go 1.18+)
pub fn is_test_name_go(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let leaf = match name.rsplit_once('.') {
        Some((_, leaf)) => leaf,
        None => name,
    };
    leaf.starts_with("Test")
        || leaf.starts_with("Benchmark")
        || leaf.starts_with("Example")
        || leaf.starts_with("Fuzz")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn go_test_file_path_heuristic() {
        assert!(is_test_file_go("user_test.go"));
        assert!(is_test_file_go("pkg/sub/user_test.go"));
        assert!(is_test_file_go("internal/api/handler_test.go"));
        assert!(!is_test_file_go("user.go"));
        assert!(!is_test_file_go("testing.go"));
        assert!(!is_test_file_go("test_user.go"));
    }

    #[test]
    fn go_test_name_heuristic() {
        assert!(is_test_name_go("TestCreateUser"));
        assert!(is_test_name_go("BenchmarkSort"));
        assert!(is_test_name_go("ExampleHello"));
        assert!(is_test_name_go("FuzzParse"));
        assert!(is_test_name_go("pkg.TestCreateUser"));
        assert!(!is_test_name_go("CreateUser"));
        assert!(!is_test_name_go("testHelper"));
        assert!(!is_test_name_go(""));
    }

    #[test]
    fn go_non_test_function_via_node() {
        let mut c: GoTestCache = HashMap::new();
        assert!(!is_test_node_go("CreateUser", "user.go", 1, &mut c));
        assert!(is_test_node_go("CreateUser", "user_test.go", 1, &mut c));
        assert!(is_test_node_go("TestCreateUser", "user.go", 1, &mut c));
    }

    #[test]
    fn detects_attr_less_test_function_via_tree_sitter() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("more.go");
        let src = "package main\n\nimport \"testing\"\n\nfunc TestSomething(t *testing.T) {}\n";
        fs::write(&path, src).expect("write");
        let file_str = path.to_string_lossy().to_string();
        let mut c: GoTestCache = HashMap::new();
        assert!(is_test_node_go("createUser", &file_str, 5, &mut c));
        assert!(!is_test_node_go("createUser", &file_str, 100, &mut c));
    }
}
