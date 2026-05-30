//! tool-call インターセプタ ── runtime が worker のアクションを適用する前のチェック。
//!
//! Claude Code の hook 隔離を runtime 化で失う分の埋め合わせ（`DESIGN.md` §10・§16.2・
//! `docs/operations.md` §2）。強制するもの:
//! - ファイル編集（`edit_file`/`write_file`）が現ノードの blast radius 内（glob マッチ）
//! - コマンド実行（`run_command`）が現ノードの `cmd_allowlist` にマッチ
//!   （`cmd_exit_0` の gate コマンドは workflow.toml 事前宣言なので暗黙許可 ── gate 評価経路は別）
//! - 作業ディレクトリ = HARNESS_HOME（`--worktree <path>` 指定時はそれ。worktree モードは scaffold）
//! - ネットワーク: `network = false`（既定）のノードでは `run_command` 前に warning
//!   （OS レベル no-network 強制は環境依存で重いので skeleton では no-op、将来 sandbox で強制）

use std::path::{Path, PathBuf};

use crate::gate::glob_match;
use crate::spec::Spec;
use crate::workflow::Node;

/// インターセプタ判定の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// 許可。
    Allow,
    /// 拒否（理由付き ── worker に返す）。
    Deny(String),
}

/// ノード文脈に紐付くインターセプタ。
pub struct Interceptor {
    node_id: String,
    /// blast radius の glob パターン群（`serves` の F-NNN の `requirement.files` ∪ ノード直 `files`）。
    blast_radius: Vec<String>,
    /// `run_command` が受け付けるコマンドパターン（ノードの `cmd_allowlist`）。
    cmd_allowlist: Vec<String>,
    /// 作業ディレクトリ（HARNESS_HOME か `--worktree`）。
    cwd: PathBuf,
    /// ネットワーク許可（既定 false）。
    network: bool,
}

impl Interceptor {
    /// ノード ＋ spec ＋ cwd からインターセプタを構築する。
    pub fn for_node(node: &Node, spec: Option<&Spec>, cwd: PathBuf) -> Self {
        Interceptor {
            node_id: node.id.clone(),
            blast_radius: node.blast_radius(spec),
            cmd_allowlist: node.cmd_allowlist.clone(),
            cwd,
            network: node.network,
        }
    }

    /// 作業ディレクトリ。
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// edit/write 対象パスが blast radius 内か。blast radius 未宣言なら制限しない（Allow）。
    pub fn check_write(&self, path: &Path) -> Verdict {
        if self.blast_radius.is_empty() {
            return Verdict::Allow;
        }
        let rel = self.relativize(path);
        for pat in &self.blast_radius {
            if glob_match(pat, &rel) {
                return Verdict::Allow;
            }
        }
        Verdict::Deny(format!(
            "編集対象 '{rel}' はノード '{}' の blast radius 外（{:?}）── 要るなら `back` で requirement.files を拡張せよ",
            self.node_id, self.blast_radius
        ))
    }

    /// 実行コマンドが `cmd_allowlist` にマッチするか。allowlist 未宣言なら拒否（明示が必要）。
    pub fn check_command(&self, cmd: &str) -> Verdict {
        if self.cmd_allowlist.is_empty() {
            return Verdict::Deny(format!(
                "ノード '{}' に cmd_allowlist が無い ── このノードでは run_command は使えない",
                self.node_id
            ));
        }
        for pat in &self.cmd_allowlist {
            if cmd_pattern_match(pat, cmd) {
                return Verdict::Allow;
            }
        }
        Verdict::Deny(format!(
            "コマンド '{cmd}' はノード '{}' の cmd_allowlist にマッチしない（{:?}）",
            self.node_id, self.cmd_allowlist
        ))
    }

    /// ネットワーク禁止のノードか（true なら `run_command` 前に warning すべき）。
    pub fn network_blocked(&self) -> bool {
        !self.network
    }

    /// read_file / edit_file 用 ── 入力パスを cwd 配下の安全なパスへ解決する。
    /// `..` での脱出と cwd 外を指す絶対パス（`/etc/passwd` / `C:\...`）を拒否する。
    ///
    /// レキシカル正規化のみ（canonicalize しない）── 理由が 2 つ:
    /// 1. write 経路では対象が未作成でもよく、canonicalize は存在しないパスで失敗する。
    /// 2. Windows の canonicalize は `\\?\C:\` verbatim prefix を付け、`check_write` の
    ///    `relativize`（非 canonical な `self.cwd` を strip）と食い違い、blast radius が
    ///    全 Deny になる。base も対象も同じレキシカル正規化で揃える。
    pub fn safe_resolve(&self, path: &str) -> Result<PathBuf, String> {
        let base = lexical_normalize(&self.cwd);
        // 絶対パス入力は join で base を置換する ── その場合も下の containment で弾かれる。
        let normalized = lexical_normalize(&base.join(path));
        if !normalized.starts_with(&base) {
            return Err(format!(
                "パス '{path}' は作業ディレクトリ '{}' の外を指す ── path traversal 拒否",
                self.cwd.display()
            ));
        }
        Ok(normalized)
    }

    fn relativize(&self, path: &Path) -> String {
        let rel = path.strip_prefix(&self.cwd).unwrap_or(path);
        rel.to_string_lossy().replace('\\', "/")
    }
}

/// パスを fs に触れずレキシカル正規化する（`.` 除去・`..` で 1 段戻す）。
/// canonicalize と違い未作成パスでも動く ── write 経路の containment 判定に使う。
fn lexical_normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                // ルート直下や prefix（C:\）を越えて戻らない ── pop が無効なら無視。
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `cmd_allowlist` パターンマッチ ── パターンとコマンドを空白でトークン分割し、
/// パターン末尾が `*` なら残りトークンを許す。各トークンは `*`/`?` を含むワイルドカード。
fn cmd_pattern_match(pattern: &str, cmd: &str) -> bool {
    let pt: Vec<&str> = pattern.split_whitespace().collect();
    let ct: Vec<&str> = cmd.split_whitespace().collect();
    let trailing_star = pt.last() == Some(&"*");
    let body = if trailing_star { &pt[..pt.len() - 1] } else { &pt[..] };
    if !trailing_star && body.len() != ct.len() {
        return false;
    }
    if trailing_star && ct.len() < body.len() {
        return false;
    }
    for (p, c) in body.iter().zip(ct.iter()) {
        if !token_match(p, c) {
            return false;
        }
    }
    true
}

/// 1 トークンのワイルドカードマッチ（`*` は 0 文字以上、`?` は 1 文字）。
fn token_match(pat: &str, s: &str) -> bool {
    glob_match(pat, s)
}
