//! Go source から testing 規約に従う test 関数を検出する。tree-sitter-go ベース、
//! `test_attrs.rs`（Rust）/ `test_attrs_ts.rs`（TS）/ `test_attrs_py.rs`（Py）と対称。
//!
//! 検出対象:
//!   a. `func TestXxx(t *testing.T)`           ── 通常 test
//!   b. `func BenchmarkXxx(b *testing.B)`      ── benchmark
//!   c. `func ExampleXxx()`                    ── godoc example
//!   d. `func FuzzXxx(f *testing.F)`           ── fuzz test (Go 1.18+)
//!
//! 検出対象外（MVP では送り）:
//!   e. `t.Run("subtest", func(t *testing.T) {...})` の inner closure
//!   f. test helper（`func helperXxx(t *testing.T)`）── 引数 type ベース判定は
//!      範囲広すぎるため name prefix のみで判定する。
//!
//! 失敗時は `Result` を返し、上位 heuristic fallback に委ねる。

use std::fs;
use std::path::Path;

use tree_sitter::{Node, Parser};

/// 指定 file の (1-origin) line が test 関数の開始行か。
#[allow(dead_code)]
pub fn is_test_function(file: &Path, line: usize) -> bool {
    match list_test_function_lines(file) {
        Ok(v) => v.contains(&line),
        Err(_) => false,
    }
}

/// 指定 file 内のすべての test 関数の (1-origin) 開始行を返す。
pub fn list_test_function_lines(file: &Path) -> Result<Vec<usize>, String> {
    let src = fs::read_to_string(file)
        .map_err(|e| format!("read {}: {}", file.display(), e))?;
    list_test_function_lines_src(&src)
}

/// ソース文字列版（unit テスト容易性のため公開）。
pub fn list_test_function_lines_src(src: &str) -> Result<Vec<usize>, String> {
    let mut parser = Parser::new();
    let lang: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
    parser
        .set_language(&lang)
        .map_err(|e| format!("set_language: {e}"))?;
    let tree = parser
        .parse(src, None)
        .ok_or_else(|| "parse failed".to_string())?;
    let bytes = src.as_bytes();
    let mut out: Vec<usize> = Vec::new();
    walk(tree.root_node(), bytes, &mut out);
    out.sort();
    out.dedup();
    Ok(out)
}

/// 再帰 walk。トップレベル `function_declaration` の name が Test/Benchmark/Example/Fuzz
/// で始まれば収集する。method_declaration（receiver 付き）は Go test 規約上 test 対象に
/// ならないので無視する。
fn walk(node: Node, src: &[u8], out: &mut Vec<usize>) {
    if node.kind() == "function_declaration" {
        if let Some(name) = fn_name(node, src) {
            if is_go_test_function_name(&name) {
                out.push(node.start_position().row + 1);
            }
        }
    }
    let mut cur = node.walk();
    for child in node.named_children(&mut cur) {
        walk(child, src, out);
    }
}

fn fn_name(node: Node, src: &[u8]) -> Option<String> {
    let name = node.child_by_field_name("name")?;
    name.utf8_text(src).ok().map(|s| s.to_string())
}

/// Go test 規約: name が Test/Benchmark/Example/Fuzz で始まれば test。
/// MVP は prefix のみ判定（厳密には Test の直後が大文字/数字/_ である必要があるが、
/// `Testing` のような偽陽性は許容範囲）。
pub fn is_go_test_function_name(name: &str) -> bool {
    name.starts_with("Test")
        || name.starts_with("Benchmark")
        || name.starts_with("Example")
        || name.starts_with("Fuzz")
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn detects_test_function() {
        let src = "package foo\n\nimport \"testing\"\n\nfunc TestCreateUser(t *testing.T) {}\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert_eq!(lines, vec![5], "TestCreateUser expected at line 5, got {:?}", lines);
    }

    #[test]
    fn detects_benchmark_function() {
        let src = "package foo\n\nimport \"testing\"\n\nfunc BenchmarkCreate(b *testing.B) {}\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert_eq!(lines, vec![5], "BenchmarkCreate expected at line 5, got {:?}", lines);
    }

    #[test]
    fn detects_example_function() {
        let src = "package foo\n\nfunc ExampleCreate() {}\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert_eq!(lines, vec![3], "ExampleCreate expected at line 3, got {:?}", lines);
    }

    #[test]
    fn detects_fuzz_function() {
        let src = "package foo\n\nimport \"testing\"\n\nfunc FuzzCreate(f *testing.F) {}\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert_eq!(lines, vec![5], "FuzzCreate expected at line 5, got {:?}", lines);
    }

    #[test]
    fn ignores_non_test_function() {
        let src = "package foo\n\nfunc helper() {}\nfunc createUser(name string) string { return name }\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert!(lines.is_empty(), "expected empty, got {:?}", lines);
    }

    #[test]
    fn name_classifier_prefixes() {
        assert!(is_go_test_function_name("TestFoo"));
        assert!(is_go_test_function_name("BenchmarkFoo"));
        assert!(is_go_test_function_name("ExampleFoo"));
        assert!(is_go_test_function_name("FuzzFoo"));
        assert!(!is_go_test_function_name("helper"));
        assert!(!is_go_test_function_name("create"));
    }
}
