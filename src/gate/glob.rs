//! 最小の glob 実装（`*` / `**` / `?` / リテラル）。gate の path 引数で使う。
//!
//! glob を含まないパスはそのまま 1 要素として返す（存在チェックは呼び出し側）。

use std::path::{Path, PathBuf};

/// glob メタ文字を含むか。
fn is_glob(p: &str) -> bool {
    p.contains('*') || p.contains('?')
}

/// パターン 1 セグメント（`**` を除く）が名前にマッチするか。
fn seg_match(pat: &str, name: &str) -> bool {
    glob_segment(pat.as_bytes(), name.as_bytes())
}

fn glob_segment(pat: &[u8], s: &[u8]) -> bool {
    match (pat.first(), s.first()) {
        (None, None) => true,
        (Some(b'*'), _) => {
            // `*` は 0 文字以上（`/` は跨がない ── セグメント内なので / は来ない）
            glob_segment(&pat[1..], s)
                || (!s.is_empty() && glob_segment(pat, &s[1..]))
        }
        (Some(b'?'), Some(_)) => glob_segment(&pat[1..], &s[1..]),
        (Some(a), Some(b)) if a == b => glob_segment(&pat[1..], &s[1..]),
        _ => false,
    }
}

/// パス全体（`/` 区切り）がパターンにマッチするか。`**` は任意段数のディレクトリ。
pub fn glob_match(pattern: &str, path: &str) -> bool {
    let pp: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let ss: Vec<&str> = path.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    match_parts(&pp, &ss)
}

fn match_parts(pat: &[&str], s: &[&str]) -> bool {
    match pat.first() {
        None => s.is_empty(),
        Some(&"**") => {
            // 0 段以上
            match_parts(&pat[1..], s)
                || (!s.is_empty() && match_parts(pat, &s[1..]))
        }
        Some(p) => {
            !s.is_empty() && seg_match(p, s[0]) && match_parts(&pat[1..], &s[1..])
        }
    }
}

/// home 基準で glob を展開し、マッチした実在ファイルの絶対パス一覧を返す。
/// glob を含まなければ `[home/p]`（存在問わず 1 要素）。
pub fn glob_paths(home: &Path, p: &str) -> Vec<PathBuf> {
    if !is_glob(p) {
        let pb = PathBuf::from(p);
        return vec![if pb.is_absolute() { pb } else { home.join(pb) }];
    }
    let mut out = Vec::new();
    walk(home, home, p, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, pattern: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        let rel = match path.strip_prefix(root) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        let ft = match e.file_type() {
            Ok(f) => f,
            Err(_) => continue,
        };
        if ft.is_dir() {
            // .git 等の重い隠しディレクトリはスキップ
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            walk(root, &path, pattern, out);
        } else if glob_match(pattern, &rel) {
            out.push(path);
        }
    }
}
