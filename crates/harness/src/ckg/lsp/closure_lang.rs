//! `find_closure` の Lang 引数版。layer 2.5 PoC で `_with_client` 版を分離。
//! BFS 全体で 1 LspClient を使い回すことで per-invocation spawn を amortize。

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use super::client::{start_and_warm_up, LspClient};
use super::closure::{ClosureNode, Direction, MAX_DEPTH};
use super::lang::Lang;
use super::refs_lang::{
    find_callers_for_lang_with_client, find_outgoing_for_lang_with_client,
};

/// `find_closure` の Lang 版 (既存 fire-and-forget API)。
pub fn find_closure_for_lang(
    qname: &str,
    depth: usize,
    direction: Direction,
    lang: Lang,
    root: &Path,
) -> Result<Vec<ClosureNode>, String> {
    let mut client = start_and_warm_up(lang, root)?;
    let result =
        find_closure_for_lang_with_client(&mut client, qname, depth, direction, lang, root);
    let _ = client.shutdown();
    result
}

/// `find_closure_for_lang` の client 再利用版。
pub fn find_closure_for_lang_with_client(
    client: &mut LspClient,
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
        nodes.extend(closure_in_for_lang(client, qname, depth, lang, root, timeout)?);
    }
    if matches!(direction, Direction::Out | Direction::Both) {
        nodes.extend(closure_out_for_lang(client, qname, depth, lang, root, timeout)?);
    }
    Ok(nodes)
}

/// direction=in: callers の BFS 展開。
fn closure_in_for_lang(
    client: &mut LspClient,
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
        let callers = match find_callers_for_lang_with_client(client, lang, root, &cur, timeout) {
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

/// direction=out: outgoing の BFS 展開。
fn closure_out_for_lang(
    client: &mut LspClient,
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
        let callees = match find_outgoing_for_lang_with_client(client, lang, root, &cur, timeout) {
            Ok(v) => v,
            Err(_) if d > 0 => continue,
            Err(e) => return Err(e),
        };
        for c in callees {
            let key = format!("{}|{}:{}", c.file, c.line, c.name);
            if !visited.insert(key) {
                continue;
            }
            out.push(ClosureNode {
                name: c.name.clone(),
                file: c.file.clone(),
                line: c.line,
                depth: d + 1,
                direction: "out".to_string(),
            });
            if d + 1 < depth && !c.name.is_empty() {
                frontier.push((c.name, d + 1));
            }
        }
    }
    Ok(out)
}
