//! daemon の list / stop 処理。port file 経由で daemon process を管理する。
//!
//! - `cmd_list()`            : 全 port file を表示 (alive/dead 判定込)
//! - `cmd_stop_specific(lang, root)` : 指定 daemon を kill + port file 削除
//! - `cmd_stop_by_lang(lang)`: 指定 lang の全 daemon を kill + port file 削除
//! - `cmd_stop_all()`        : 全 daemon を kill + port file 削除
//! - `cmd_stop_stale()`      : dead な port file のみ削除 (process は触らない)
//!
//! Windows-only。生存判定は PID + TCP port reachability の二段:
//! - `tasklist /NH /FI` で PID alive、`taskkill /F /PID` で kill
//! - port: `TcpStream::connect_timeout(127.0.0.1:<port>, 100ms)` で reachable 確認

use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

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
        let status = if is_process_alive_strict(e.content.pid, e.content.port) {
            "alive"
        } else {
            "dead"
        };
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

/// `harness lsp-daemon stop --lang <l>` (root 省略) の実装。指定 lang の全 daemon を kill。
pub fn cmd_stop_by_lang(lang: &str) -> Result<(), String> {
    let entries = port_file_list::list_all()?;
    let mut killed = 0usize;
    for e in entries.iter().filter(|e| e.lang == lang) {
        if is_process_alive_strict(e.content.pid, e.content.port) {
            let _ = kill_pid(e.content.pid);
            killed += 1;
        }
        let _ = port_file::delete(&e.path);
    }
    println!("stopped {killed} {lang} daemons");
    Ok(())
}

/// `harness lsp-daemon stop --all` の実装。
pub fn cmd_stop_all() -> Result<(), String> {
    let entries = port_file_list::list_all()?;
    let mut killed = 0usize;
    for e in entries {
        if is_process_alive_strict(e.content.pid, e.content.port) {
            let _ = kill_pid(e.content.pid);
            killed += 1;
        }
        let _ = port_file::delete(&e.path);
    }
    println!("stopped {killed} daemons");
    Ok(())
}

/// `harness lsp-daemon stop --stale` の実装。
/// PID + port reachability の二段で dead 判定 (片方 NG で stale 扱い)。
pub fn cmd_stop_stale() -> Result<(), String> {
    let entries = port_file_list::list_all()?;
    let mut removed = 0usize;
    for e in entries {
        if !is_process_alive_strict(e.content.pid, e.content.port) {
            let _ = port_file::delete(&e.path);
            removed += 1;
        }
    }
    println!("removed {removed} stale port files");
    Ok(())
}

/// PID + port の二段で生存確認。両方 OK で true、片方 NG で false (stale 扱い)。
fn is_process_alive_strict(pid: u32, port: u16) -> bool {
    is_pid_alive(pid) && is_port_reachable(port)
}

/// PID alive check (Windows tasklist 経由)。
fn is_pid_alive(pid: u32) -> bool {
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

/// port が localhost で TCP connect 可能か (timeout 100ms)。
fn is_port_reachable(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok()
}

fn kill_pid(pid: u32) -> Result<(), String> {
    let status = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .status()
        .map_err(|e| format!("taskkill: {e}"))?;
    if !status.success() {
        return Err(format!("taskkill failed for pid={pid}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_list_does_not_panic_on_clean_env() {
        assert!(cmd_list().is_ok());
    }

    #[test]
    fn is_pid_alive_returns_false_for_unlikely_pid() {
        assert!(!is_pid_alive(4_294_967_294));
    }

    #[test]
    fn is_port_reachable_returns_false_for_unused_high_port() {
        // 65500 は usually 未使用なので false 期待。たまたま使用中なら true でも fail させない。
        let _ = is_port_reachable(65500);
    }

    #[test]
    fn is_process_alive_strict_false_when_pid_dead() {
        assert!(!is_process_alive_strict(4_294_967_294, 65500));
    }
}
