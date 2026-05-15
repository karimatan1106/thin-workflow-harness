//! Go 専用 test 判定ヘルパ。`tested_lang.rs` から分離して 200 行ルールを保つ。
//!
//! Phase A MVP は heuristic のみ（tree-sitter-go 連携は次バッチ送り）:
//! - path: ファイル名が `*_test.go` で終わる（Go の test ファイル規約）
//! - name: 関数名が `Test*` / `Benchmark*` / `Example*` / `Fuzz*` で始まる
//!
//! tree-sitter なしで判定するため file 単位キャッシュは不要。

use super::uri::percent_decode;

/// Go: path heuristic または name heuristic のどちらかで test と判定。
pub fn is_test_node_go(name: &str, file: &str) -> bool {
    is_test_file_go(file) || is_test_name_go(name)
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
        // method-style qname
        assert!(is_test_name_go("pkg.TestCreateUser"));
        assert!(!is_test_name_go("CreateUser"));
        assert!(!is_test_name_go("testHelper"));
        assert!(!is_test_name_go(""));
    }

    #[test]
    fn go_non_test_function() {
        // file も name も非 test なら hit しない
        assert!(!is_test_node_go("CreateUser", "user.go"));
        // file 規約 hit
        assert!(is_test_node_go("CreateUser", "user_test.go"));
        // name 規約 hit
        assert!(is_test_node_go("TestCreateUser", "user.go"));
    }
}
