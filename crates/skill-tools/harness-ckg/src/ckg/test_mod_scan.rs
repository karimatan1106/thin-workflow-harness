//! `#[cfg(test)] mod` 親階層判定 ── test_attrs.rs 補助モジュール。
//!
//! `#[test]` 系 attr が直接付いていない関数でも、`#[cfg(test)] mod tests { ... }`
//! の内側に居れば「test 用 helper / 関数」とみなす判定を提供する。
//!
//! tree-sitter-rust の grammar 上、 `#[cfg(test)] mod tests { ... }` は
//!   `mod_item` の前 sibling として `attribute_item` が並び、
//!   attribute_item の中の `attribute` ノードに
//!     - path 識別子（`identifier "cfg"`）
//!     - 引数（`token_tree` または `meta_arguments` ─ grammar 版差あり）
//!   が含まれる。引数文字列を一度 utf8_text() で取り、その中に
//!   `test` 単語が居るかを単純に検査する。`cfg_attr(test, ...)` のような
//!   複雑形は MVP 範囲外（attr 直接検出 or heuristic に任せる）。

use std::fs;
use std::path::Path;

use tree_sitter::{Node, Parser};

/// 指定 file 内のすべての `#[cfg(test)] mod` ブロックの (1-origin) 行 range を返す。
///
/// 各要素は `(start_row + 1, end_row + 1)`（inclusive）。
/// 失敗時（read 不可・parse 不可）は `Err`。
pub fn list_cfg_test_mod_ranges(file: &Path) -> Result<Vec<(usize, usize)>, String> {
    let src = fs::read_to_string(file)
        .map_err(|e| format!("read {}: {}", file.display(), e))?;
    list_cfg_test_mod_ranges_src(&src)
}

/// ソース文字列版（unit test 容易性のため公開）。
pub fn list_cfg_test_mod_ranges_src(src: &str) -> Result<Vec<(usize, usize)>, String> {
    let mut parser = Parser::new();
    let lang = tree_sitter_rust::LANGUAGE.into();
    parser
        .set_language(&lang)
        .map_err(|e| format!("set_language: {e}"))?;
    let tree = parser
        .parse(src, None)
        .ok_or_else(|| "parse failed".to_string())?;
    let bytes = src.as_bytes();
    let mut out: Vec<(usize, usize)> = Vec::new();
    walk_collect_mods(tree.root_node(), bytes, &mut out);
    out.sort();
    out.dedup();
    Ok(out)
}

/// 指定 file の (1-origin) line が `#[cfg(test)] mod` の内側にあるかを返す。
///
/// 失敗時は `false`（呼び出し側で heuristic fallback に任せる）。
pub fn is_inside_cfg_test_mod(file: &Path, line: usize) -> bool {
    let ranges = match list_cfg_test_mod_ranges(file) {
        Ok(v) => v,
        Err(_) => return false,
    };
    line_in_ranges(line, &ranges)
}

/// 与えた line が ranges のどれかに含まれるか（inclusive）。
pub fn line_in_ranges(line: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|(s, e)| *s <= line && line <= *e)
}

/// 再帰 walk して mod_item を全列挙、各 mod の前 sibling attr を見て cfg(test) を判定。
fn walk_collect_mods(node: Node, src: &[u8], out: &mut Vec<(usize, usize)>) {
    let mut cur = node.walk();
    for child in node.named_children(&mut cur) {
        if child.kind() == "mod_item" && has_preceding_cfg_test_attr(child, src) {
            let start = child.start_position().row + 1;
            let end = child.end_position().row + 1;
            out.push((start, end));
        }
        walk_collect_mods(child, src, out);
    }
}

/// mod_item の直前 sibling を遡って `#[cfg(test)]` attr があるか確認する。
/// doc comment は skip。
fn has_preceding_cfg_test_attr(m: Node, src: &[u8]) -> bool {
    let mut sib = m.prev_named_sibling();
    while let Some(n) = sib {
        match n.kind() {
            "attribute_item" | "inner_attribute_item" => {
                if attribute_item_is_cfg_test(n, src) {
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

/// attribute_item を文字列化して `cfg(test)` パターンを検出。
///
/// 厳密 AST マッチではなく、attr_item 全体の utf8_text を見て `cfg` 識別子と
/// その引数領域に `test` token を含むかを判定する。`#[cfg(test)]` /
/// `#[ cfg ( test ) ]` / 改行入りも吸収。 `cfg_attr(test, ...)` は false
/// （MVP は `cfg(test)` のみ）。
fn attribute_item_is_cfg_test(attr_item: Node, src: &[u8]) -> bool {
    let text = match attr_item.utf8_text(src) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let stripped: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    // `#[cfg(test)]` / `#![cfg(test)]` / 入れ子無しを最優先で。
    if stripped.contains("cfg(test)") && !stripped.contains("cfg_attr(") {
        return true;
    }
    false
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn detects_single_cfg_test_mod_range() {
        let src = "
#[cfg(test)]
mod tests {
    fn helper() {}
}
";
        let ranges = list_cfg_test_mod_ranges_src(src).expect("parse ok");
        assert_eq!(ranges.len(), 1, "expected 1 cfg(test) mod, got {:?}", ranges);
        let (start, end) = ranges[0];
        assert!(start <= 3 && end >= 5, "range {:?} should cover mod body", (start, end));
    }

    #[test]
    fn line_in_ranges_inside_and_outside() {
        let ranges = vec![(3usize, 5usize)];
        assert!(line_in_ranges(3, &ranges));
        assert!(line_in_ranges(4, &ranges));
        assert!(line_in_ranges(5, &ranges));
        assert!(!line_in_ranges(2, &ranges));
        assert!(!line_in_ranges(6, &ranges));
    }

    #[test]
    fn ignores_plain_mod_without_cfg_test() {
        let src = "
mod foo {
    fn bar() {}
}
";
        let ranges = list_cfg_test_mod_ranges_src(src).expect("parse ok");
        assert!(ranges.is_empty(), "expected empty for plain mod, got {:?}", ranges);
    }

    #[test]
    fn ignores_cfg_other_than_test() {
        let src = "
#[cfg(feature = \"foo\")]
mod bar {
}
";
        let ranges = list_cfg_test_mod_ranges_src(src).expect("parse ok");
        assert!(ranges.is_empty(), "expected empty for cfg(feature=...), got {:?}", ranges);
    }

    #[test]
    fn ignores_cfg_attr_test() {
        // cfg_attr(test, ...) は MVP 範囲外 ── range に入れない。
        let src = "
#[cfg_attr(test, derive(Debug))]
mod bar {
}
";
        let ranges = list_cfg_test_mod_ranges_src(src).expect("parse ok");
        assert!(ranges.is_empty(), "expected empty for cfg_attr(test,...), got {:?}", ranges);
    }

    #[test]
    fn whitespace_tolerant_cfg_test() {
        let src = "
#[ cfg ( test ) ]
mod tests {
}
";
        let ranges = list_cfg_test_mod_ranges_src(src).expect("parse ok");
        assert_eq!(ranges.len(), 1, "should detect whitespace-spaced cfg(test), got {:?}", ranges);
    }
}
