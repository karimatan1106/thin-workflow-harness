//! `harness impacted-by <qname>` ── 変更影響範囲評価のための薄いラッパ。
//!
//! `find_closure(direction=in)` を呼ぶだけ。semantic は「qname を変更すると
//! 影響を受ける呼び出し元集合」だが、データ的には `direction=in` の transitive
//! closure と同一。出力ノードは `ImpactedNode` に詰め替える（direction フィールド
//! を落として impacted-by 専用ビューにする）。
//!
//! 既定 depth=3（closure より深い、impacted-by は広域評価が主目的）。

use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use super::closure::{find_closure, ClosureNode, Direction, MAX_DEPTH};
use super::client::LspClient;
use super::closure_lang::{find_closure_for_lang, find_closure_for_lang_with_client};
use super::lang::Lang;

/// impacted-by 結果の 1 ノード（direction を落とした closure ノードのビュー）。
#[derive(Debug, Clone, Serialize)]
pub struct ImpactedNode {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub depth: usize,
}

impl From<ClosureNode> for ImpactedNode {
    fn from(n: ClosureNode) -> Self {
        Self {
            name: n.name,
            file: n.file,
            line: n.line,
            depth: n.depth,
        }
    }
}

/// impacted-by の既定 depth。
pub const DEFAULT_DEPTH: usize = 3;

/// `harness impacted-by <qname>` 本体（Rust 固定の後方互換ラッパ）。
pub fn find_impacted_by(
    server_cmd: &str,
    root: &Path,
    qname: &str,
    depth: usize,
    timeout: Duration,
) -> Result<Vec<ImpactedNode>, String> {
    let depth = depth.clamp(1, MAX_DEPTH);
    let nodes = find_closure(server_cmd, root, qname, depth, Direction::In, timeout)?;
    Ok(nodes.into_iter().map(ImpactedNode::from).collect())
}

/// Lang 引数版 ── `find_closure_for_lang(direction=in)` の薄いラッパ。
pub fn find_impacted_by_for_lang(
    qname: &str,
    depth: usize,
    lang: Lang,
    root: &Path,
) -> Result<Vec<ImpactedNode>, String> {
    let depth = depth.clamp(1, MAX_DEPTH);
    let nodes = find_closure_for_lang(qname, depth, Direction::In, lang, root)?;
    Ok(nodes.into_iter().map(ImpactedNode::from).collect())
}

/// `find_impacted_by_for_lang` の client 再利用版 (layer 2.5 PoC)。
pub fn find_impacted_by_for_lang_with_client(
    client: &mut LspClient,
    qname: &str,
    depth: usize,
    lang: Lang,
    root: &Path,
) -> Result<Vec<ImpactedNode>, String> {
    let depth = depth.clamp(1, MAX_DEPTH);
    let nodes = find_closure_for_lang_with_client(
        client, qname, depth, Direction::In, lang, root,
    )?;
    Ok(nodes.into_iter().map(ImpactedNode::from).collect())
}
