//! TS 専用 test 判定ヘルパ。`tested_lang.rs` から分離して 200 行ルールを保つ。
//!
//! - tree-sitter ベース attr 検出 (`test_attrs_ts::list_test_function_lines`)
//! - path heuristic / name heuristic
//!   の 3 段で `is_test_node_ts` を判定する。

use std::collections::HashMap;
use std::path::PathBuf;

use super::uri::percent_decode;
use crate::ckg::test_attrs_ts::list_test_function_lines as list_test_function_lines_ts;

/// TS 用の file 単位 test fn line キャッシュ。
pub type TsTestCache = HashMap<PathBuf, Option<Vec<usize>>>;

/// TS: tree-sitter attr → file path heuristic → name heuristic の 3 段。
pub fn is_test_node_ts(name: &str, file: &str, line: usize, cache: &mut TsTestCache) -> bool {
    if let Some(lines) = ts_attr_entries(file, cache) {
        if lines.contains(&line) {
            return true;
        }
    }
    is_test_file_ts(file) || is_test_name_ts(name)
}

fn ts_attr_entries<'a>(file: &str, cache: &'a mut TsTestCache) -> Option<&'a Vec<usize>> {
    let decoded = percent_decode(file);
    let path = PathBuf::from(&decoded);
    if !path.exists() {
        return None;
    }
    // 現状 `.ts` のみ tree-sitter で attr 走査（`.tsx` は MVP 対象外、
    // heuristic fallback で吸収する）。
    let ext_ok = path.extension().and_then(|s| s.to_str()) == Some("ts");
    if !ext_ok {
        return None;
    }
    if !cache.contains_key(&path) {
        let parsed = list_test_function_lines_ts(&path).ok();
        cache.insert(path.clone(), parsed);
    }
    cache.get(&path).and_then(|x| x.as_ref())
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
