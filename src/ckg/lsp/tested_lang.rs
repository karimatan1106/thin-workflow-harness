//! `find_tested_by` の Lang 版。`find_closure_for_lang(direction=in)` の結果から
//! `is_test_node_for_lang(node, lang)` で test 関数のみフィルタする。
//!
//! - Rust: attr ベース (`test_attrs::list_test_function_lines`) +
//!   `cfg(test) mod` 親階層判定 (`test_mod_scan`) + heuristic fallback
//! - TS: heuristic のみ MVP
//!   path: `__tests__/` / `test/` / `tests/` / `.test.ts` / `.spec.ts`
//!   name: `test_` 開始 / `_test` 終わり / `describe` / `it`
//!   ── `@jest` / `@vitest` attr 検出や `describe()` / `it()` block 内側判定は
//!   tree-sitter-typescript 連携が必要、次バッチ送り。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::closure::{ClosureNode, Direction, MAX_DEPTH};
use super::closure_lang::find_closure_for_lang;
use super::lang::Lang;
use super::tested::TestedNode;
use super::uri::percent_decode;
use crate::ckg::test_attrs::list_test_function_lines;
use crate::ckg::test_mod_scan::{line_in_ranges, list_cfg_test_mod_ranges};

/// `harness tested-by <qname> --lang ...` 本体。
pub fn find_tested_by_for_lang(
    qname: &str,
    depth: usize,
    lang: Lang,
    root: &Path,
) -> Result<Vec<TestedNode>, String> {
    let depth = depth.clamp(1, MAX_DEPTH);
    let nodes = find_closure_for_lang(qname, depth, Direction::In, lang, root)?;
    let mut cache: RustTestCache = HashMap::new();
    let filtered: Vec<TestedNode> = nodes
        .into_iter()
        .filter(|n| is_test_node_for_lang(n, lang, &mut cache))
        .map(TestedNode::from)
        .collect();
    Ok(filtered)
}

/// 1 file 分の Rust attr 情報キャッシュ（test fn 開始行 + cfg(test) mod range）。
#[derive(Debug, Default, Clone)]
struct RustTestInfo {
    test_lines: Vec<usize>,
    cfg_mod_ranges: Vec<(usize, usize)>,
}

/// Rust 用の file 単位 attr キャッシュ。
type RustTestCache = HashMap<PathBuf, Option<RustTestInfo>>;

/// Lang ごとの test ノード判定。
fn is_test_node_for_lang(
    node: &ClosureNode,
    lang: Lang,
    cache: &mut RustTestCache,
) -> bool {
    match lang {
        Lang::Rust => is_test_node_rust(&node.name, &node.file, node.line, cache),
        Lang::Ts => is_test_node_ts(&node.name, &node.file),
    }
}

/// Rust: attr (#[test] 系) → cfg(test) mod 内側 → heuristic fallback の 3 段。
fn is_test_node_rust(name: &str, file: &str, line: usize, cache: &mut RustTestCache) -> bool {
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

fn is_test_file_rust(file: &str) -> bool {
    let norm = file.replace('\\', "/");
    if norm.ends_with("_test.rs") || norm.ends_with("_tests.rs") {
        return true;
    }
    norm.starts_with("tests/") || norm.contains("/tests/")
}

fn is_test_name_rust(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let leaf = match name.rsplit_once("::") {
        Some((_, leaf)) => leaf,
        None => name,
    };
    leaf.starts_with("test_") || leaf.ends_with("_test")
}

/// TS: heuristic 単独 MVP。path / name の単純パターンで判定。
/// `@jest` / `@vitest` decorator や `describe()` block 内側判定は次バッチ。
fn is_test_node_ts(name: &str, file: &str) -> bool {
    is_test_file_ts(file) || is_test_name_ts(name)
}

fn is_test_file_ts(file: &str) -> bool {
    let decoded = percent_decode(file);
    let norm = decoded.replace('\\', "/").to_ascii_lowercase();
    if norm.ends_with(".test.ts")
        || norm.ends_with(".test.tsx")
        || norm.ends_with(".spec.ts")
        || norm.ends_with(".spec.tsx")
    {
        return true;
    }
    norm.contains("/__tests__/")
        || norm.starts_with("__tests__/")
        || norm.contains("/test/")
        || norm.starts_with("test/")
        || norm.contains("/tests/")
        || norm.starts_with("tests/")
}

fn is_test_name_ts(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let leaf = match name.rsplit_once('.') {
        Some((_, leaf)) => leaf,
        None => name,
    };
    leaf.starts_with("test_")
        || leaf.ends_with("_test")
        || leaf == "describe"
        || leaf == "it"
        || leaf == "test"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(name: &str, file: &str) -> ClosureNode {
        ClosureNode {
            name: name.to_string(),
            file: file.to_string(),
            line: 1,
            depth: 1,
            direction: "in".to_string(),
        }
    }

    #[test]
    fn ts_test_file_path_heuristic() {
        let mut c = HashMap::new();
        assert!(is_test_node_for_lang(&mk("foo", "src/user.test.ts"), Lang::Ts, &mut c));
        assert!(is_test_node_for_lang(&mk("foo", "src/user.spec.ts"), Lang::Ts, &mut c));
        assert!(is_test_node_for_lang(&mk("foo", "src/__tests__/user.ts"), Lang::Ts, &mut c));
        assert!(is_test_node_for_lang(&mk("foo", "tests/user.ts"), Lang::Ts, &mut c));
        assert!(!is_test_node_for_lang(&mk("foo", "src/user.ts"), Lang::Ts, &mut c));
    }

    #[test]
    fn ts_test_name_heuristic() {
        let mut c = HashMap::new();
        assert!(is_test_node_for_lang(&mk("describe", "src/user.ts"), Lang::Ts, &mut c));
        assert!(is_test_node_for_lang(&mk("it", "src/user.ts"), Lang::Ts, &mut c));
        assert!(is_test_node_for_lang(&mk("test_foo", "src/user.ts"), Lang::Ts, &mut c));
        assert!(is_test_node_for_lang(&mk("my_test", "src/user.ts"), Lang::Ts, &mut c));
        assert!(!is_test_node_for_lang(&mk("createUser", "src/user.ts"), Lang::Ts, &mut c));
    }
}
