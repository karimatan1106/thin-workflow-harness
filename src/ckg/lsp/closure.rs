//! `harness closure <qname>` の LSP 層 ── refs/callers の transitive 合成。
//!
//! 公開 API は 2 つ:
//!   - `find_closure`: rust-analyzer 固定の後方互換ラッパ（`server_cmd` /
//!     `timeout` を受け取るが Rust 固定で `find_closure_for_lang` に委譲）
//!   - `find_closure_for_lang`: 多言語版 (closure_lang.rs)
//!
//! Direction / ClosureNode / MAX_DEPTH は当ファイルが SSOT で公開し、
//! closure_lang.rs / impacted.rs / tested*.rs が共有する。

use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use super::closure_lang::find_closure_for_lang;
use super::lang::Lang;

/// 方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction { In, Out, Both }

impl Direction {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "in" => Ok(Direction::In),
            "out" => Ok(Direction::Out),
            "both" => Ok(Direction::Both),
            other => Err(format!("unknown direction: {other} (in|out|both)")),
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self { Direction::In => "in", Direction::Out => "out", Direction::Both => "both" }
    }
}

/// closure 結果の 1 ノード。
#[derive(Debug, Clone, Serialize)]
pub struct ClosureNode {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub depth: usize,
    pub direction: String,
}

/// 上限 depth。指数爆発防止。
pub const MAX_DEPTH: usize = 5;

/// `harness closure <qname>` ── rust-analyzer 固定の後方互換ラッパ。
///
/// `server_cmd` / `timeout` は API 互換のため受け取るが、内部では
/// `find_closure_for_lang(Lang::Rust)` に委譲する（timeout は固定 60s）。
pub fn find_closure(
    _server_cmd: &str,
    root: &Path,
    qname: &str,
    depth: usize,
    direction: Direction,
    _timeout: Duration,
) -> Result<Vec<ClosureNode>, String> {
    find_closure_for_lang(qname, depth, direction, Lang::Rust, root)
}
