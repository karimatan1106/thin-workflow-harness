//! port file 一覧取得 (admin/list 用)。
//!
//! `daemon-<lang>-<hash>.port` を cache_dir から enumerate し、
//! ファイル名から lang/hash を分離して PortFileEntry を返す。
//! parse 失敗した file は skip (best-effort)。

use std::fs;
use std::path::PathBuf;

use super::port_file::{self, PortFileContent};

/// list_all() で返す 1 entry。
#[derive(Debug, Clone)]
pub struct PortFileEntry {
    pub path: PathBuf,
    pub lang: String,
    pub workspace_hash: String,
    pub content: PortFileContent,
}

/// cache_dir 配下の `daemon-<lang>-<hash>.port` を全部読み出す。
pub fn list_all() -> Result<Vec<PortFileEntry>, String> {
    let dir = port_file::cache_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let rd = fs::read_dir(&dir).map_err(|e| format!("read_dir {}: {}", dir.display(), e))?;
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !name.starts_with("daemon-") || !name.ends_with(".port") {
            continue;
        }
        let body = &name["daemon-".len()..name.len() - ".port".len()];
        let sep = match body.rfind('-') {
            Some(i) => i,
            None => continue,
        };
        let lang = body[..sep].to_string();
        let workspace_hash = body[sep + 1..].to_string();
        if let Ok(content) = port_file::read(&path) {
            out.push(PortFileEntry { path, lang, workspace_hash, content });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_all_is_ok_even_when_empty() {
        assert!(list_all().is_ok());
    }

    #[test]
    fn list_all_parses_filename_into_lang_and_hash() {
        let dir = match port_file::cache_dir() {
            Ok(d) => d,
            Err(_) => return,
        };
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("daemon-rust-deadbeefcafef00d.port");
        let c = PortFileContent { pid: 999_999, port: 1, started_at_ms: 1 };
        if port_file::write(&path, &c).is_err() {
            return;
        }
        let entries = list_all().expect("list_all");
        let found = entries.iter().find(|e| e.path == path).cloned();
        let _ = port_file::delete(&path);
        let f = found.expect("entry for fake port file");
        assert_eq!(f.lang, "rust");
        assert_eq!(f.workspace_hash, "deadbeefcafef00d");
        assert_eq!(f.content.pid, 999_999);
    }
}
