//! Python source から pytest 風の test 関数を検出する。tree-sitter-python ベース、
//! `test_attrs.rs`（Rust）/ `test_attrs_ts.rs`（TS）と対称。
//! 検出: a) `def test_*`, b) `class Test*` の def, c) `@pytest.mark.*` decorator,
//! d) `@pytest.fixture` は除外。失敗時は `Result` を返し上位 heuristic に委ねる。

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
    let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
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

/// 再帰 walk。`function_definition` は名前 heuristic、`decorated_definition` は
/// `pytest.mark.*` decorator のみ採用（`@pytest.fixture` 等は除外）。
fn walk(node: Node, src: &[u8], out: &mut Vec<usize>) {
    match node.kind() {
        "function_definition" => {
            if let Some(name) = fn_name(node, src) {
                if is_test_fn_name(&name) {
                    out.push(node.start_position().row + 1);
                }
            }
        }
        "decorated_definition" => {
            if has_pytest_mark_decorator(node, src) {
                if let Some(fn_node) = inner_function(node) {
                    out.push(fn_node.start_position().row + 1);
                }
            }
            // decorated_definition 配下の function_definition は decorator 判定で
            // 採用済 or 不採用が確定しているので、通常の walk からは skip する。
            let mut cur = node.walk();
            for child in node.named_children(&mut cur) {
                if child.kind() == "function_definition" {
                    continue;
                }
                walk(child, src, out);
            }
            return;
        }
        _ => {}
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

fn is_test_fn_name(name: &str) -> bool {
    name.starts_with("test_") || name == "test" || name.ends_with("_test")
}

fn inner_function(decorated: Node) -> Option<Node> {
    let mut cur = decorated.walk();
    #[allow(clippy::manual_find)]
    for child in decorated.named_children(&mut cur) {
        if child.kind() == "function_definition" {
            return Some(child);
        }
    }
    None
}

fn has_pytest_mark_decorator(decorated: Node, src: &[u8]) -> bool {
    let mut cur = decorated.walk();
    for child in decorated.named_children(&mut cur) {
        if child.kind() == "decorator" && decorator_is_pytest_mark(child, src) {
            return true;
        }
    }
    false
}

/// `@pytest.mark.parametrize(...)` や `@pytest.mark.skip` を判定。
/// `@pytest.fixture` は false。
fn decorator_is_pytest_mark(decorator: Node, src: &[u8]) -> bool {
    let target = match decorator.named_child(0) {
        Some(n) => n,
        None => return false,
    };
    let path = match decorator_target_path(target, src) {
        Some(p) => p,
        None => return false,
    };
    is_pytest_decorator(&path)
}

/// decorator の対象 node から `.` 連結 path を取り出す。
/// `call` の場合は呼び出される対象（attribute / identifier）を辿る。
fn decorator_target_path(node: Node, src: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" => node.utf8_text(src).ok().map(|s| s.to_string()),
        "attribute" => {
            let object = node.child_by_field_name("object")?;
            let attr = node.child_by_field_name("attribute")?;
            let obj_path = decorator_target_path(object, src)?;
            let attr_name = attr.utf8_text(src).ok()?.to_string();
            Some(format!("{obj_path}.{attr_name}"))
        }
        "call" => {
            let func = node.child_by_field_name("function")?;
            decorator_target_path(func, src)
        }
        _ => None,
    }
}

/// `pytest.mark.*` で始まる path だけ true。`pytest.fixture` 等は false。
pub fn is_pytest_decorator(path: &str) -> bool {
    path.starts_with("pytest.mark.")
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn detects_top_level_test_function() {
        let src = "def test_foo():\n    pass\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert_eq!(lines, vec![1], "top-level test_foo expected at line 1");
    }

    #[test]
    fn detects_test_class_method() {
        let src = "class TestBar:\n    def test_qux(self):\n        pass\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert!(lines.contains(&2), "test_qux expected at line 2, got {:?}", lines);
    }

    #[test]
    fn detects_pytest_parametrize_decorator() {
        let src = "@pytest.mark.parametrize(\"x\", [1, 2])\ndef test_param(x):\n    pass\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert!(lines.contains(&2), "test_param expected at line 2, got {:?}", lines);
    }

    #[test]
    fn ignores_pytest_fixture() {
        let src = "@pytest.fixture\ndef setup():\n    pass\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert!(lines.is_empty(), "pytest.fixture must not be detected, got {:?}", lines);
    }

    #[test]
    fn ignores_non_test_function() {
        let src = "def helper():\n    pass\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert!(lines.is_empty(), "expected empty, got {:?}", lines);
    }

    #[test]
    fn is_pytest_decorator_examples() {
        assert!(is_pytest_decorator("pytest.mark.parametrize"));
        assert!(is_pytest_decorator("pytest.mark.skip"));
        assert!(is_pytest_decorator("pytest.mark.xfail"));
        assert!(!is_pytest_decorator("pytest.fixture"));
        assert!(!is_pytest_decorator("functools.lru_cache"));
    }
}
