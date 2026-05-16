//! Port file 規約 -- daemon が listen 中の port を file 経由で client に通知。
//!
//! 配置:
//! - Windows: %LOCALAPPDATA%\thin-workflow-harness\daemon-<lang>-<hash>.port
//! - Unix:    ~/.cache/thin-workflow-harness/daemon-<lang>-<hash>.port
//!
//! Content (3 行):
//! ```text
//! <PID>
//! <PORT>
//! <started_at_unix_epoch_ms>
//! ```
//!
//! daemon 起動時に書き出し、shutdown 時に best-effort 削除。
//! stale でも client 側で connect 失敗 → 削除 → auto-spawn フォールバックする。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cache directory for port files.
pub fn cache_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let local = std::env::var("LOCALAPPDATA")
            .map_err(|e| format!("LOCALAPPDATA: {e}"))?;
        Ok(PathBuf::from(local).join("thin-workflow-harness"))
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME").map_err(|e| format!("HOME: {e}"))?;
        Ok(PathBuf::from(home).join(".cache").join("thin-workflow-harness"))
    }
}

/// canonical workspace path → 16-hex DefaultHasher digest.
///
/// 64bit 衝突確率 ~10^-19 で実用十分。canonicalize 失敗時は元 path をそのまま使う。
pub fn workspace_hash(root: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let s = canonical.to_string_lossy().to_string();
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// `<cache_dir>/daemon-<lang>-<hash>.port`
pub fn port_file_path(lang: &str, root: &Path) -> Result<PathBuf, String> {
    let dir = cache_dir()?;
    let h = workspace_hash(root);
    Ok(dir.join(format!("daemon-{lang}-{h}.port")))
}

/// Port file の中身。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortFileContent {
    pub pid: u32,
    pub port: u16,
    pub started_at_ms: u128,
}

/// 3 行 (pid / port / started_at_ms) を書き出す。親 dir も create_dir_all。
pub fn write(path: &Path, content: &PortFileContent) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let body = format!(
        "{}\n{}\n{}\n",
        content.pid, content.port, content.started_at_ms
    );
    fs::write(path, body).map_err(|e| format!("write port file {}: {}", path.display(), e))
}

/// 3 行を parse して `PortFileContent` を返す。形式不一致は Err。
pub fn read(path: &Path) -> Result<PortFileContent, String> {
    let body = fs::read_to_string(path)
        .map_err(|e| format!("read port file {}: {}", path.display(), e))?;
    let mut lines = body.lines();
    let pid_s = lines
        .next()
        .ok_or_else(|| format!("port file {}: missing PID line", path.display()))?;
    let port_s = lines
        .next()
        .ok_or_else(|| format!("port file {}: missing PORT line", path.display()))?;
    let started_s = lines
        .next()
        .ok_or_else(|| format!("port file {}: missing started_at line", path.display()))?;
    let pid: u32 = pid_s
        .trim()
        .parse()
        .map_err(|e| format!("port file {}: parse pid: {e}", path.display()))?;
    let port: u16 = port_s
        .trim()
        .parse()
        .map_err(|e| format!("port file {}: parse port: {e}", path.display()))?;
    let started_at_ms: u128 = started_s
        .trim()
        .parse()
        .map_err(|e| format!("port file {}: parse started_at: {e}", path.display()))?;
    Ok(PortFileContent { pid, port, started_at_ms })
}

/// 削除 (存在しない場合は Ok)。
pub fn delete(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path)
            .map_err(|e| format!("delete {}: {}", path.display(), e))
    } else {
        Ok(())
    }
}

/// Unix epoch milliseconds.
pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_hash_is_stable() {
        let p = std::env::temp_dir();
        let a = workspace_hash(&p);
        let b = workspace_hash(&p);
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn write_read_roundtrip() {
        let dir = std::env::temp_dir().join(format!("twh-port-test-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.port");
        let c = PortFileContent { pid: 12345, port: 49500, started_at_ms: 1_700_000_000_000 };
        write(&path, &c).expect("write");
        let got = read(&path).expect("read");
        assert_eq!(got, c);
        let _ = delete(&path);
    }

    #[test]
    fn delete_is_idempotent() {
        let path = std::env::temp_dir().join(format!("twh-nonexistent-{}.port", now_ms()));
        delete(&path).expect("delete nonexistent");
        delete(&path).expect("delete twice");
    }

    #[test]
    fn read_bad_format_errors() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("twh-bad-{}.port", now_ms()));
        std::fs::write(&path, "not\nvalid\ncontent\n").unwrap();
        assert!(read(&path).is_err());
        let _ = delete(&path);
    }

}
