//! daemon の list / stop 処理。port file 経由で daemon process を管理する。
//!
//! - `cmd_list()`        : 全 port file を表示 (alive/dead 判定込)
//! - `cmd_stop_specific(lang, root)` : 指定 daemon を kill + port file 削除
//! - `cmd_stop_all()`    : 全 daemon を kill + port file 削除
//! - `cmd_stop_stale()`  : dead な port file のみ削除 (process は触らない)
//!
//! process 生存 / kill は OS 標準コマンドにフォールバック:
//! - Windows: `tasklist /NH /FI` で生存判定、`taskkill /F /PID` で kill
//! - Unix:    `kill -0 <pid>` で生存判定、`kill -TERM` -> 3s wait -> `kill -KILL`

use std::path::Path;

use crate::lsp_daemon::port_file;
use crate::lsp_daemon::port_file_list;

/// `harness lsp-daemon list` の実装。
pub fn cmd_list() -> Result<(), String> {
    let entries = port_file_list::list_all()?;
    println!(
        "{:<6} {:<18} {:<7} {:<6} {:<16} STATUS",
        "LANG", "WS_HASH", "PID", "PORT", "STARTED_AT_MS"
    );
    for e in entries {
        let status = if is_process_alive(e.content.pid) { "alive" } else { "dead" };
        println!(
            "{:<6} {:<18} {:<7} {:<6} {:<16} {}",
            e.lang,
            e.workspace_hash,
            e.content.pid,
            e.content.port,
            e.content.started_at_ms,
            status
        );
    }
    Ok(())
}

/// `harness lsp-daemon stop --lang <l> --root <r>` の実装。
pub fn cmd_stop_specific(lang: &str, root: &Path) -> Result<(), String> {
    let path = port_file::port_file_path(lang, root)?;
    let content = port_file::read(&path)
        .map_err(|e| format!("no port file for lang={lang} root={}: {e}", root.display()))?;
    let _ = kill_pid(content.pid);
    let _ = port_file::delete(&path);
    println!("stopped daemon: lang={lang} pid={} port={}", content.pid, content.port);
    Ok(())
}

/// `harness lsp-daemon stop --all` の実装。
pub fn cmd_stop_all() -> Result<(), String> {
    let entries = port_file_list::list_all()?;
    let mut killed = 0usize;
    for e in entries {
        if is_process_alive(e.content.pid) {
            let _ = kill_pid(e.content.pid);
            killed += 1;
        }
        let _ = port_file::delete(&e.path);
    }
    println!("stopped {killed} daemons");
    Ok(())
}

/// `harness lsp-daemon stop --stale` の実装。
pub fn cmd_stop_stale() -> Result<(), String> {
    let entries = port_file_list::list_all()?;
    let mut removed = 0usize;
    for e in entries {
        if !is_process_alive(e.content.pid) {
            let _ = port_file::delete(&e.path);
            removed += 1;
        }
    }
    println!("removed {removed} stale port files");
    Ok(())
}

fn is_process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("tasklist")
            .args(["/NH", "/FI", &format!("PID eq {pid}")])
            .output();
        match out {
            Ok(o) => {
                let s = String::from_utf8_lossy(&o.stdout);
                // tasklist は該当無しでも "INFO: No tasks ..." を返すので pid 文字列の存在確認。
                s.contains(&pid.to_string())
            }
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn kill_pid(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let status = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .status()
            .map_err(|e| format!("taskkill: {e}"))?;
        if !status.success() {
            return Err(format!("taskkill failed for pid={pid}"));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
        std::thread::sleep(std::time::Duration::from_secs(3));
        let still = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if still {
            let _ = std::process::Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .status();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_list_does_not_panic_on_clean_env() {
        assert!(cmd_list().is_ok());
    }

    #[test]
    fn is_process_alive_returns_false_for_unlikely_pid() {
        assert!(!is_process_alive(4_294_967_294));
    }
}
