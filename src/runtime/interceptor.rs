//! tool-call インターセプタ（scaffold のみ）。
//!
//! Phase 1 で本格化する。runtime が worker のツール呼び出しを仲介し、
//! edit/write が blast-radius（ノードの `files` / `serves` から導出）内か、
//! 実行コマンドが `cmd_allowlist` 内か、cwd が worktree か、ネットワークアクセス禁止
//! （`network = true` のノードを除く）か ── を強制する。
//! Claude Code の hook 隔離を runtime 化で失う分の埋め合わせ
//! （`DESIGN.md` §10 のトレードオフ「hook 隔離を失う」、`docs/operations.md` §2）。
//!
//! skeleton ではスクリプト worker が raw ファイル編集（`edit` ツール）を出さない
//! ── `harness` コマンド相当のアクション（record-artifact / report-evidence /
//! request-transition / create_file）だけ ── ので、ここは no-op の scaffold に留める。
//! blast-radius 編集制限の本実装は Phase 1。

use std::path::Path;

use crate::workflow::Node;

/// インターセプタ判定の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// 許可。
    Allow,
    /// 拒否（理由付き）。
    Deny(String),
}

/// ノード文脈に紐付くインターセプタ（scaffold）。
pub struct Interceptor<'a> {
    #[allow(dead_code)]
    node: &'a Node,
    /// blast-radius のパス接頭辞群（ノードの `files`/`serves` から Phase 1 で導出）。
    blast_radius: Vec<String>,
}

impl<'a> Interceptor<'a> {
    /// ノードからインターセプタを構築する。skeleton では blast_radius は空のまま。
    pub fn for_node(node: &'a Node) -> Self {
        Interceptor { node, blast_radius: Vec::new() }
    }

    /// edit/write 対象パスが許可されるか（skeleton: 常に Allow）。
    /// Phase 1: `path` が `blast_radius` のいずれかの接頭辞配下か、cwd=worktree かを検査する。
    pub fn check_write(&self, _path: &Path) -> Verdict {
        // Phase 1 で本格化。
        Verdict::Allow
    }

    /// 実行コマンドが `cmd_allowlist` 内か（skeleton: 常に Allow）。
    /// Phase 1: コマンド先頭トークンが node.cmd_allowlist にマッチするかを検査する。
    pub fn check_command(&self, _cmd: &str) -> Verdict {
        // Phase 1 で本格化。
        Verdict::Allow
    }

    /// blast-radius に登録されている接頭辞数（Phase 1 で実体が入る）。
    #[allow(dead_code)]
    pub fn blast_radius_len(&self) -> usize {
        self.blast_radius.len()
    }
}
