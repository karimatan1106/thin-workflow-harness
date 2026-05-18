//! Rust source ファイルから `#[test]` 系 attribute を持つ関数を検出する。
//!
//! tree-sitter-rust ベース。outline.rs と独立。
//! MVP 検出対象 attr（path の最終 segment を見る）:
//!   - `test`                       (#[test])
//!   - `tokio::test`                (#[tokio::test])
//!   - `async_std::test`            (#[async_std::test])
//!   - `rstest::rstest` / `rstest`  (#[rstest])
//!   - `test_case::test_case` / `test_case`
//!   - `tracing_test::traced_test` / `traced_test`
//!
//! `#[cfg(test)] mod` 内側の判定は次バッチ送り（MVP は attr 直接検出のみ）。
//! 失敗時（parse 不可・read 不可）は heuristic fallback できるよう
//! `Result` で返し、上位は `unwrap_or_default` でフォールバックする。

use std::fs;
use std::path::Path;

use tree_sitter::{Node, Parser};

/// 「これは test attribute」と判定する attribute path の最終 segment。
const TEST_ATTR_LEAVES: &[&str] = &[
    "test",
    "rstest",
    "test_case",
    "traced_test",
];

/// attr path（identifier / scoped_identifier の文字列表現）を test 判定する。
///
/// `tokio::test` / `async_std::test` 等は path 最終 segment `test` で hit。
/// `rstest::rstest` / `test_case::test_case` 等も同様。
fn is_test_attr_path(path: &str) -> bool {
    let trimmed = path.trim();
    if TEST_ATTR_LEAVES.contains(&trimmed) {
        return true;
    }
    let leaf = match trimmed.rsplit_once("::") {
        Some((_, l)) => l,
        None => trimmed,
    };
    TEST_ATTR_LEAVES.contains(&leaf)
}

/// 指定 file の (1-origin) line にある関数が test attribute を持つか。
///
/// 失敗時は `false`（heuristic fallback に任せる）。
pub fn is_test_function(file: &Path, line: usize) -> bool {
    let lines = match list_test_function_lines(file) {
        Ok(v) => v,
        Err(_) => return false,
    };
    lines.contains(&line)
}

/// 指定 file 内のすべての test 関数の (1-origin) 開始行を返す。
///
/// tree-sitter で `function_item` を全走査し、直前に並ぶ `attribute_item` の
/// path を見て test attr 該当を判定する。
pub fn list_test_function_lines(file: &Path) -> Result<Vec<usize>, String> {
    let src = fs::read_to_string(file)
        .map_err(|e| format!("read {}: {}", file.display(), e))?;
    list_test_function_lines_src(&src)
}

/// ソース文字列版（unit テスト容易性のため公開）。
pub fn list_test_function_lines_src(src: &str) -> Result<Vec<usize>, String> {
    let mut parser = Parser::new();
    let lang = tree_sitter_rust::LANGUAGE.into();
    parser
        .set_language(&lang)
        .map_err(|e| format!("set_language: {e}"))?;
    let tree = parser
        .parse(src, None)
        .ok_or_else(|| "parse failed".to_string())?;
    let bytes = src.as_bytes();
    let mut out: Vec<usize> = Vec::new();
    walk_collect(tree.root_node(), bytes, &mut out);
    out.sort();
    out.dedup();
    Ok(out)
}

/// 再帰的に walk して `function_item` の前にある test attr を確認する。
fn walk_collect(node: Node, src: &[u8], out: &mut Vec<usize>) {
    let mut cur = node.walk();
    for child in node.named_children(&mut cur) {
        if child.kind() == "function_item" && has_preceding_test_attr(child, src) {
            out.push(child.start_position().row + 1);
        }
        walk_collect(child, src, out);
    }
}

/// 与えた function_item の直前 sibling を遡って test attr があるか確認する。
///
/// tree-sitter-rust では function の attribute は同一親の前 sibling として
/// `attribute_item` ノードで現れる。間に doc comment が挟まることはあるので
/// line_comment / block_comment は skip して更に遡る。
fn has_preceding_test_attr(func: Node, src: &[u8]) -> bool {
    let mut sib = func.prev_named_sibling();
    while let Some(n) = sib {
        match n.kind() {
            "attribute_item" | "inner_attribute_item" => {
                if attribute_item_is_test(n, src) {
                    return true;
                }
            }
            "line_comment" | "block_comment" => {}
            _ => return false,
        }
        sib = n.prev_named_sibling();
    }
    false
}

/// attribute_item ノードから attr path 文字列を取り出して test 判定する。
fn attribute_item_is_test(attr_item: Node, src: &[u8]) -> bool {
    let mut cur = attr_item.walk();
    for ch in attr_item.named_children(&mut cur) {
        if ch.kind() == "attribute" {
            if let Some(path_node) = ch.named_child(0) {
                let text = path_node.utf8_text(src).unwrap_or("").trim();
                if is_test_attr_path(text) {
                    return true;
                }
            }
            if let Some(path_node) = ch.child_by_field_name("path") {
                let text = path_node.utf8_text(src).unwrap_or("").trim();
                if is_test_attr_path(text) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn detects_plain_test() {
        let src = "\n#[test]\nfn alpha() {}\n\nfn beta() {}\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert_eq!(lines, vec![3]);
    }

    #[test]
    fn detects_tokio_and_async_std_and_rstest() {
        let src = "\n#[tokio::test]\nasync fn a_tokio() {}\n\n#[async_std::test]\nasync fn a_async() {}\n\n#[rstest]\nfn r_basic() {}\n\n#[test_case::test_case(1)]\nfn r_case() {}\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert_eq!(lines.len(), 4, "expected 4 test fns, got {:?}", lines);
    }

    #[test]
    fn ignores_non_test_attrs() {
        let src = "\n#[inline]\nfn not_test() {}\n\n#[derive(Debug)]\nstruct S;\n";
        let lines = list_test_function_lines_src(src).expect("parse ok");
        assert!(lines.is_empty(), "expected empty, got {:?}", lines);
    }

    #[test]
    fn is_test_attr_path_matrix() {
        assert!(is_test_attr_path("test"));
        assert!(is_test_attr_path("tokio::test"));
        assert!(is_test_attr_path("async_std::test"));
        assert!(is_test_attr_path("rstest"));
        assert!(is_test_attr_path("rstest::rstest"));
        assert!(is_test_attr_path("test_case"));
        assert!(is_test_attr_path("tracing_test::traced_test"));
        assert!(!is_test_attr_path("inline"));
        assert!(!is_test_attr_path("derive"));
        assert!(!is_test_attr_path("cfg"));
    }
}
