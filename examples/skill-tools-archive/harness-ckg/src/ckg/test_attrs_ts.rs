//! TypeScript source から `describe(...)` / `it(...)` / `test(...)` block
//! 内側の関数を test 関数として検出する。
//!
//! tree-sitter-typescript ベース。`test_attrs.rs`（Rust）と対構造。
//! MVP 検出対象:
//!   a. `describe(...)` / `it(...)` / `test(...)` の call_expression そのもの
//!   b. describe ブロック内側の function
//!      (`function_declaration` / `arrow_function` / `function_expression` /
//!      `method_definition`)
//!
//! jest / vitest の import 文 filter は行わない。"describe があるファイル中の
//! function は test" という heuristic で十分。
//!
//! 失敗時は `Result` で返し、上位の heuristic fallback に委ねる。

use std::fs;
use std::path::Path;

use tree_sitter::{Node, Parser};

/// `describe` / `it` / `test` 等、callback を受ける test runner 関数名。
const TEST_RUNNER_FNS: &[&str] = &[
    "describe", "it", "test", "suite", "context",
    "fdescribe", "xdescribe", "fit", "xit",
];

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
    let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    parser
        .set_language(&lang)
        .map_err(|e| format!("set_language: {e}"))?;
    let tree = parser
        .parse(src, None)
        .ok_or_else(|| "parse failed".to_string())?;
    let bytes = src.as_bytes();
    let mut out: Vec<usize> = Vec::new();
    walk_collect(tree.root_node(), bytes, false, &mut out);
    out.sort();
    out.dedup();
    Ok(out)
}

/// 再帰 walk。describe/it/test の call_expression を踏むと内側を inside_test
/// にして再帰。inside_test 中の function-like ノードも検出する。
fn walk_collect(node: Node, src: &[u8], inside_test: bool, out: &mut Vec<usize>) {
    let here_is_test_call =
        node.kind() == "call_expression" && call_is_test_runner(node, src);
    if here_is_test_call {
        out.push(node.start_position().row + 1);
    }
    if inside_test && is_function_like(node.kind()) {
        out.push(node.start_position().row + 1);
    }
    let child_inside = inside_test || here_is_test_call;
    let mut cur = node.walk();
    for child in node.named_children(&mut cur) {
        walk_collect(child, src, child_inside, out);
    }
}

/// `call_expression` ノードが `describe(...)` / `it(...)` / `test(...)` か。
/// `describe.skip(...)` 等の member access も対応する。
fn call_is_test_runner(node: Node, src: &[u8]) -> bool {
    let func = match node.child_by_field_name("function") {
        Some(n) => n,
        None => return false,
    };
    match leaf_callee_name(func, src) {
        Some(name) => TEST_RUNNER_FNS.contains(&name.as_str()),
        None => false,
    }
}

/// callee node から「最終的に呼ばれる識別子名」を取り出す。
fn leaf_callee_name(func: Node, src: &[u8]) -> Option<String> {
    match func.kind() {
        "identifier" | "property_identifier" => {
            func.utf8_text(src).ok().map(|s| s.to_string())
        }
        "member_expression" => {
            let obj = func.child_by_field_name("object")?;
            leaf_callee_name(obj, src)
        }
        "parenthesized_expression" => leaf_callee_name(func.named_child(0)?, src),
        _ => None,
    }
}

/// この kind は「関数的」か。describe 内側で test 扱いする対象。
fn is_function_like(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "function_expression"
            | "arrow_function"
            | "method_definition"
            | "generator_function_declaration"
            | "generator_function"
    )
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn detects_describe_block() {
        let src = r#"
describe(user, () => {
  it(foo, () => {
    expect(1).toBe(1);
  });
});
"#;
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert!(lines.contains(&2), "line 2 expected, got {:?}", lines);
        assert!(lines.contains(&3), "line 3 expected, got {:?}", lines);
    }

    #[test]
    fn detects_arrow_inside_describe() {
        let src = r#"
describe(arith, () => {
  const helper = (x: number) => x + 1;
  it(adds, () => {
    expect(helper(1)).toBe(2);
  });
});
"#;
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert!(lines.contains(&3), "helper arrow at line 3 expected, got {:?}", lines);
    }

    #[test]
    fn ignores_non_test_function() {
        let src = r#"
function createUser(name: string) {
  return { name };
}

const formatName = (n: string) => n.toUpperCase();
"#;
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert!(lines.is_empty(), "expected empty, got {:?}", lines);
    }

    #[test]
    fn detects_describe_dot_skip() {
        let src = r#"
describe.skip(legacy, () => {
  it(old, () => {});
});
"#;
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert!(!lines.is_empty(), "expected describe.skip detected, got {:?}", lines);
    }
}
