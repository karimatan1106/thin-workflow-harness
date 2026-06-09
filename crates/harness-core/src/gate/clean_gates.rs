//! clean-state gate: git_clean ── working tree が clean handoff 状態かを検証する (L12)。
//!
//! 講義 L12「毎セッションは clean state で終える」の deterministic な実装。5 次元
//! (build / test / progress / artifacts 除去 / startup) のうち、build/test は
//! `cmd_exit_0` が、progress は event log が担う。本 gate は **artifacts 除去**
//! 次元 ── working tree に debug/temp/orphan な残骸が残っていないこと ── を担う。
//!
//! args:
//!   - `untracked_only` (bool, 既定 false): true なら追跡済みの変更 (正当な実装 diff)
//!     は許し、未追跡ファイル (`??` ── debug/temp 残骸) だけを fail にする。run が
//!     commit しない段階の終端 gate ではこちらを使う。
//!   - `ignore` (str, `|` 区切り): 除外する path パターン (glob または前方一致)。
//!     glob は段数厳密 ── nested を消すなら `**/` を付ける (`*.tmp` は top-level のみ)。
//!     例 `"target|node_modules|**/*.tmp|.harness/state/**"`。

use std::process::Command;

use super::{arg_bool, arg_str, glob_match, GateCtx, GateResult};

/// path が ignore パターンにマッチするか。glob メタ文字があれば glob、無ければ前方一致。
fn ignored(path: &str, pat: &str) -> bool {
    if pat.contains('*') || pat.contains('?') {
        glob_match(pat, path)
    } else {
        // 前方一致 ("target" は "target/foo" を除外、ただし "targetx" は除外しない)
        path == pat || path.starts_with(&format!("{pat}/"))
    }
}

pub(super) fn git_clean(args: &toml::Table, ctx: &GateCtx) -> GateResult {
    let untracked_only = arg_bool(args, "untracked_only").unwrap_or(false);
    let ignore: Vec<&str> = arg_str(args, "ignore")
        .map(|s| s.split('|').map(|x| x.trim()).filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();

    let out = match Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(ctx.home)
        .output()
    {
        Ok(o) => o,
        Err(e) => return GateResult::fail(format!("git status 実行失敗: {e}")),
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return GateResult::fail(format!(
            "git status 失敗: {}",
            err.lines().next().unwrap_or("(no stderr)")
        ));
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let dirty = collect_dirty(&text, untracked_only, &ignore);
    if dirty.is_empty() {
        let scope = if untracked_only { "未追跡残骸なし" } else { "working tree clean" };
        GateResult::ok(scope.to_string())
    } else {
        let preview: Vec<&str> = dirty.iter().take(5).map(String::as_str).collect();
        GateResult::fail(format!("clean でない {} 件: {}", dirty.len(), preview.join(", ")))
    }
}

/// porcelain 出力を fold して「咎めるべき汚れ」を集める純関数 (テスト対象)。
fn collect_dirty(porcelain: &str, untracked_only: bool, ignore: &[&str]) -> Vec<String> {
    let mut dirty = Vec::new();
    for line in porcelain.lines() {
        if line.len() < 4 {
            continue;
        }
        let status = &line[..2];
        let path = line[3..].trim();
        // untracked_only=true なら追跡済み変更 (実装 diff) は許し、未追跡 (??) だけ咎める
        if untracked_only && status != "??" {
            continue;
        }
        if ignore.iter().any(|pat| ignored(path, pat)) {
            continue;
        }
        dirty.push(format!("{} {}", status.trim(), path));
    }
    dirty
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_prefix_and_glob() {
        assert!(ignored("target/debug/x", "target"));
        assert!(!ignored("targetx/y", "target"));
        // glob_match の段数セマンティクス: `*.tmp` は top-level のみ、nested は `**/` が要る
        assert!(ignored("scratch.tmp", "*.tmp"));
        assert!(!ignored("a/b.tmp", "*.tmp"));
        assert!(ignored("a/b.tmp", "**/*.tmp"));
        assert!(ignored("src/scratch.log", "**/*.log"));
    }

    #[test]
    fn clean_tree_is_empty() {
        assert!(collect_dirty("", false, &[]).is_empty());
    }

    #[test]
    fn untracked_only_skips_tracked_changes() {
        let p = " M src/lib.rs\n?? scratch.tmp\n";
        // strict: 両方咎める
        assert_eq!(collect_dirty(p, false, &[]).len(), 2);
        // untracked_only: 未追跡 scratch.tmp だけ
        let d = collect_dirty(p, true, &[]);
        assert_eq!(d.len(), 1);
        assert!(d[0].contains("scratch.tmp"));
    }

    #[test]
    fn ignore_filters_out_match() {
        let p = "?? target/junk\n?? real_orphan.txt\n";
        let d = collect_dirty(p, true, &["target"]);
        assert_eq!(d.len(), 1);
        assert!(d[0].contains("real_orphan.txt"));
    }
}
