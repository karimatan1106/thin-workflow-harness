//! `find_closure` の Lang 引数版。既存 `closure::*` の多言語ラッパで、
//! `Lang` から server コマンドを解決して spawn する。
//!
//! direction=in は `find_callers_for_lang` を BFS 再帰、out は 1 段 refs。
//! visited set + depth clamp(1, MAX_DEPTH) は既存実装と同一。
//! 200 行制約のため closure.rs から分離。

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use super::closure::{ClosureNode, Direction, MAX_DEPTH};
use super::lang::Lang;
use super::refs_lang::{find_callers_for_lang, find_refs_for_lang};

/// `find_closure` の Lang 版。
///
/// - direction=in: `find_callers_for_lang` の transitive BFS（depth まで）
/// - direction=out: `find_refs_for_lang` で 1 段（depth=1 相当）
/// - direction=both: 両方
pub fn find_closure_for_lang(
    qname: &str,
    depth: usize,
    direction: Direction,
    lang: Lang,
    root: &Path,
) -> Result<Vec<ClosureNode>, String> {
    let depth = depth.clamp(1, MAX_DEPTH);
    let timeout = Duration::from_secs(60);
    let mut nodes: Vec<ClosureNode> = Vec::new();
    if matches!(direction, Direction::In | Direction::Both) {
        nodes.extend(closure_in_for_lang(qname, depth, lang, root, timeout)?);
    }
    if matches!(direction, Direction::Out | Direction::Both) {
        nodes.extend(closure_out_for_lang(qname, lang, root, timeout)?);
    }
    Ok(nodes)
}

/// direction=in: `find_callers_for_lang` を BFS で transitive 展開。
/// 各 depth で visited な qname を再追跡しない（uri+line key）。
fn closure_in_for_lang(
    qname: &str,
    depth: usize,
    lang: Lang,
    root: &Path,
    timeout: Duration,
) -> Result<Vec<ClosureNode>, String> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut out: Vec<ClosureNode> = Vec::new();
    let mut frontier: Vec<(String, usize)> = vec![(qname.to_string(), 0)];
    while let Some((cur, d)) = frontier.pop() {
        if d >= depth {
            continue;
        }
        let callers = match find_callers_for_lang(lang, root, &cur, timeout) {
            Ok(v) => v,
            Err(_) if d > 0 => continue,
            Err(e) => return Err(e),
        };
        for c in callers {
            let key = format!("{}|{}:{}", c.file, c.line, c.name);
            if !visited.insert(key) {
                continue;
            }
            out.push(ClosureNode {
                name: c.name.clone(),
                file: c.file.clone(),
                line: c.line,
                depth: d + 1,
                direction: "in".to_string(),
            });
            if d + 1 < depth && !c.name.is_empty() {
                frontier.push((c.name, d + 1));
            }
        }
    }
    Ok(out)
}

/// direction=out: `find_refs_for_lang` で 1 段（MVP）。既存 closure_out 相当。
fn closure_out_for_lang(
    qname: &str,
    lang: Lang,
    root: &Path,
    timeout: Duration,
) -> Result<Vec<ClosureNode>, String> {
    let refs = find_refs_for_lang(lang, root, qname, timeout)?;
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<ClosureNode> = Vec::with_capacity(refs.len());
    for r in refs {
        let key = format!("{}:{}", r.file, r.line);
        if !seen.insert(key) {
            continue;
        }
        out.push(ClosureNode {
            name: String::new(),
            file: r.file,
            line: r.line,
            depth: 1,
            direction: "out".to_string(),
        });
    }
    Ok(out)
}
