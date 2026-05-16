//! LSP subprocess spawn のクロスプラットフォーム実装。
//!
//! Windows では npm global install で `typescript-language-server` が `.cmd` shim と
//! して prefix 直下に置かれる。Rust の `Command::new` は PATHEXT 解決をしないため、
//! bare 名で `Command::new("typescript-language-server")` すると `program not found` で
//! 失敗する。さらに npm prefix （例 `~/.npm-global`）が Windows %PATH% に
//! 入っていないことも多く、`cmd /c typescript-language-server` でも見つからない。
//!
//! そこで以下の 3 段階フォールバックを実装する:
//!   1. `Command::new(cmd).args(args).spawn()`   ── PATH 直接解決
//!   2. `cmd /c <cmd> <args...>`                 ── PATHEXT 経由で `.cmd` shim を解決
//!   3. `npm config get prefix` で得た prefix を PATH に append し、もう一度 2 を試す
//!
//! 非 Windows では (1) のみで十分。

use std::process::{Child, Command, Stdio};

/// `Command::new(cmd).args(args).spawn()` の薄ラッパ。stdio を 3 本ともパイプ。
pub(super) fn spawn_child(cmd: &str, args: &[String]) -> Result<Child, std::io::Error> {
    Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
}

/// Windows fallback: `cmd /c <cmd> <args...>` 経由で起動する。
/// 直接の `cmd /c` で見つからない場合、`npm config get prefix` の出力を PATH に足して
/// もう 1 回試す（npm prefix が Windows %PATH% に入っていない環境で有効）。
#[cfg(windows)]
pub(super) fn spawn_via_cmd(cmd: &str, args: &[String]) -> Result<Child, String> {
    match spawn_via_cmd_inner(cmd, args) {
        Ok(c) => Ok(c),
        Err(e1) => {
            if let Some(prefix) = npm_prefix() {
                augment_path_with(&prefix);
                return spawn_via_cmd_inner(cmd, args)
                    .map_err(|e2| format!("{e1}; after npm prefix: {e2}"));
            }
            Err(format!("{e1}"))
        }
    }
}

#[cfg(windows)]
fn spawn_via_cmd_inner(cmd: &str, args: &[String]) -> Result<Child, std::io::Error> {
    let mut full = Vec::with_capacity(args.len() + 2);
    full.push("/c".to_string());
    full.push(cmd.to_string());
    full.extend(args.iter().cloned());
    Command::new("cmd")
        .args(&full)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
}

/// `npm config get prefix` を実行して、global install 先のディレクトリを返す。
/// 失敗・空応答は None。
#[cfg(windows)]
fn npm_prefix() -> Option<String> {
    let out = Command::new("cmd")
        .args(["/c", "npm", "config", "get", "prefix"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 現在の `PATH` に `prefix` を先頭追加する（プロセス内のみ、永続化しない）。
#[cfg(windows)]
fn augment_path_with(prefix: &str) {
    let cur = std::env::var("PATH").unwrap_or_default();
    if cur.split(';').any(|p| p.eq_ignore_ascii_case(prefix)) {
        return;
    }
    let new_path = format!("{prefix};{cur}");
    std::env::set_var("PATH", new_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_child_returns_err_for_unknown_binary() {
        let r = spawn_child("definitely_not_a_real_binary_xyz", &[]);
        assert!(r.is_err());
    }

    #[cfg(windows)]
    #[test]
    fn augment_path_with_prepends_once() {
        // 既存 PATH を退避
        let saved = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", "C:\\a;C:\\b");
        augment_path_with("C:\\new");
        let now = std::env::var("PATH").unwrap_or_default();
        assert!(now.starts_with("C:\\new;"), "got {now}");
        // 二重追加防止
        augment_path_with("C:\\new");
        let after = std::env::var("PATH").unwrap_or_default();
        assert_eq!(after.matches("C:\\new").count(), 1);
        // restore
        std::env::set_var("PATH", saved);
    }
}
