//! Rust source ファイルの outline 抽出。tree-sitter-rust ベース。
//!
//! 出力単位 = [`Symbol`]。トップレベル + impl 内のメソッドだけ抽出する。
//! 深いネスト（fn 内 fn 等）は走査しない。

use std::fs;
use std::path::Path;

use serde::Serialize;
use tree_sitter::{Node, Parser};

/// Symbol の種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Mod,
    Const,
    Static,
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SymbolKind::Function => "fn",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Mod => "mod",
            SymbolKind::Const => "const",
            SymbolKind::Static => "static",
        }
    }
}

/// 抽出した1シンボル。`children` は impl 内のメソッド等のネスト用。
#[derive(Debug, Clone, Serialize)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Symbol>,
}

/// ファイルパスから outline を抽出。
pub fn outline_file(path: &Path) -> Result<Vec<Symbol>, String> {
    let src = fs::read_to_string(path)
        .map_err(|e| format!("read {}: {}", path.display(), e))?;
    outline_source(&src)
}

/// ソース文字列から outline を抽出。
pub fn outline_source(src: &str) -> Result<Vec<Symbol>, String> {
    let mut parser = Parser::new();
    let lang = tree_sitter_rust::LANGUAGE.into();
    parser
        .set_language(&lang)
        .map_err(|e| format!("set_language: {e}"))?;
    let tree = parser
        .parse(src, None)
        .ok_or_else(|| "parse failed".to_string())?;
    let root = tree.root_node();
    let bytes = src.as_bytes();

    let mut out: Vec<Symbol> = Vec::new();
    let mut cur = root.walk();
    for child in root.named_children(&mut cur) {
        if let Some(sym) = node_to_symbol(child, bytes) {
            out.push(sym);
        }
    }
    Ok(out)
}

/// 1ノードを Symbol に変換。対象外ノードは None。impl は内部メソッドを children に詰める。
pub(crate) fn node_to_symbol(node: Node, src: &[u8]) -> Option<Symbol> {
    let kind = match node.kind() {
        "function_item" => SymbolKind::Function,
        "struct_item" => SymbolKind::Struct,
        "enum_item" => SymbolKind::Enum,
        "trait_item" => SymbolKind::Trait,
        "impl_item" => SymbolKind::Impl,
        "mod_item" => SymbolKind::Mod,
        "const_item" => SymbolKind::Const,
        "static_item" => SymbolKind::Static,
        _ => return None,
    };
    let name = extract_name(node, src, kind);
    let signature = extract_signature(node, src, kind);
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let children = if matches!(kind, SymbolKind::Impl | SymbolKind::Trait) {
        collect_body_children(node, src)
    } else {
        Vec::new()
    };
    Some(Symbol { kind, name, start_line, end_line, signature, children })
}

/// `name` フィールド or `type` フィールドからシンボル名を取り出す。
/// impl は impl 対象型を name 代わりにする。
fn extract_name(node: Node, src: &[u8], kind: SymbolKind) -> String {
    if matches!(kind, SymbolKind::Impl) {
        if let Some(ty) = node.child_by_field_name("type") {
            return slice(ty, src);
        }
        return "<impl>".to_string();
    }
    if let Some(n) = node.child_by_field_name("name") {
        return slice(n, src);
    }
    "<anon>".to_string()
}

/// 1行サマリのシグネチャ。
/// - function: `fn NAME(params) -> ret`
/// - struct/enum/trait/mod/const/static: 1行目を整形（body 直前まで）
fn extract_signature(node: Node, src: &[u8], kind: SymbolKind) -> String {
    if matches!(kind, SymbolKind::Function) {
        return function_signature(node, src);
    }
    head_until_body(node, src, kind)
}

fn function_signature(node: Node, src: &[u8]) -> String {
    let name = node
        .child_by_field_name("name")
        .map(|n| slice(n, src))
        .unwrap_or_else(|| "<anon>".to_string());
    let params = node
        .child_by_field_name("parameters")
        .map(|n| slice(n, src))
        .unwrap_or_else(|| "()".to_string());
    let ret = node
        .child_by_field_name("return_type")
        .map(|n| format!(" -> {}", slice(n, src)));
    let mut s = format!("fn {}{}", name, params);
    if let Some(r) = ret {
        s.push_str(&r);
    }
    collapse_ws(&s)
}

/// body の `{` 直前までを取って整形。body が無いノード（const 等）は ; を除いた全文。
fn head_until_body(node: Node, src: &[u8], kind: SymbolKind) -> String {
    let full = slice(node, src);
    if let Some(idx) = full.find('{') {
        let head = full[..idx].trim_end();
        return collapse_ws(head);
    }
    let head = full.trim_end_matches(';').trim_end();
    let _ = kind;
    collapse_ws(head)
}

/// impl/trait の body 直下の named children を Symbol 化。
fn collect_body_children(node: Node, src: &[u8]) -> Vec<Symbol> {
    let Some(body) = node.child_by_field_name("body") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut cur = body.walk();
    for ch in body.named_children(&mut cur) {
        if let Some(sym) = node_to_symbol(ch, src) {
            out.push(sym);
        }
    }
    out
}

fn slice(node: Node, src: &[u8]) -> String {
    node.utf8_text(src).unwrap_or("").to_string()
}

/// 連続空白・改行を 1 半角空白に潰す（シグネチャ 1 行表示用）。
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    out.trim().to_string()
}
