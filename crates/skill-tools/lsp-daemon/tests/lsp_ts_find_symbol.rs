//! `find_symbol_for_lang` 用 TypeScript integration test。
//!
//! `typescript-language-server` が PATH に無ければ skip。
//! 在環境では sample_workspace_ts/ 配下の `User` クラスが取れることを確認する。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thin_workflow_harness_ckg::ckg::lsp::{find_symbol_for_lang, Lang};

fn fixture_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/sample_workspace_ts");
    p
}

/// `typescript-language-server --version` が 0 で返れば true。
/// Windows では `.cmd` shim を直接 spawn できないので `cmd /c` 経由で再試行し、
/// それでも見つからない場合は `npm config get prefix` を見て PATH に append する。
fn ts_lsp_available() -> bool {
    if try_ts_version() {
        return true;
    }
    #[cfg(windows)]
    {
        // npm global install で `.cmd` shim は prefix 直下に置かれる
        if let Ok(out) = Command::new("cmd")
            .args(["/c", "npm", "config", "get", "prefix"])
            .output()
        {
            if out.status.success() {
                let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !prefix.is_empty() {
                    let cur = std::env::var("PATH").unwrap_or_default();
                    let sep = ";";
                    let new_path = format!("{prefix}{sep}{cur}");
                    std::env::set_var("PATH", &new_path);
                    return try_ts_version();
                }
            }
        }
    }
    false
}

fn try_ts_version() -> bool {
    let direct = Command::new("typescript-language-server")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if direct {
        return true;
    }
    #[cfg(windows)]
    {
        return Command::new("cmd")
            .args(["/c", "typescript-language-server", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    }
    #[allow(unreachable_code)]
    false
}

#[test]
fn find_symbol_user_in_ts_workspace() {
    if !ts_lsp_available() {
        eprintln!("skip: typescript-language-server が PATH に無いため skip");
        return;
    }
    let root = fixture_root();
    let syms = find_symbol_for_lang(
        Lang::Ts,
        &root,
        "User",
        None,
        Duration::from_secs(60),
    )
    .expect("find_symbol_for_lang ok");
    // ts_bootstrap::warm_up_ts_workspace で didOpen を流す改修以降、
    // sample_workspace_ts では `User` が必ず取れる前提（"No Project" は出ない）。
    assert!(
        !syms.is_empty(),
        "workspace/symbol returned empty even after didOpen — tsserver project not loaded?"
    );
    let hit = syms.iter().any(|s| s.name.contains("User"));
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert!(hit, "User not found in: {names:?}");
}
