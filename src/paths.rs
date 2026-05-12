//! HARNESS_HOME 解決と state/ skills/ のパス計算。

use std::env;
use std::fs;
use std::path::PathBuf;

/// HARNESS_HOME があればそれ、無ければ CWD。
pub fn harness_home() -> PathBuf {
    match env::var_os("HARNESS_HOME") {
        Some(v) => PathBuf::from(v),
        None => env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

/// state ディレクトリ。存在しなければ作成する。
pub fn state_dir() -> Result<PathBuf, String> {
    let dir = harness_home().join("state");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("state ディレクトリ作成失敗 {}: {e}", dir.display()))?;
    }
    Ok(dir)
}

/// skills ディレクトリ。
pub fn skills_dir() -> PathBuf {
    harness_home().join("skills")
}

/// skill ファイルの絶対パス（存在チェックはしない）。
pub fn skill_path(skill_filename: &str) -> PathBuf {
    skills_dir().join(skill_filename)
}

/// run のイベントログパス。
pub fn event_log_path(run_id: &str) -> Result<PathBuf, String> {
    Ok(state_dir()?.join(format!("{run_id}.jsonl")))
}

/// run_id を解決する。explicit → HARNESS_RUN → state/ 内で最新の *.jsonl の stem。
pub fn resolve_run_id(explicit: Option<&str>) -> Result<String, String> {
    if let Some(r) = explicit {
        return Ok(r.to_string());
    }
    if let Some(r) = env::var_os("HARNESS_RUN") {
        let s = r.to_string_lossy().to_string();
        if !s.is_empty() {
            return Ok(s);
        }
    }
    latest_run_id()?.ok_or_else(|| "no runs found; run `harness start \"...\"` first".to_string())
}

/// state/ 内で最終更新が最新の *.jsonl の stem を返す。
fn latest_run_id() -> Result<Option<String>, String> {
    let dir = state_dir()?;
    let mut best: Option<(std::time::SystemTime, String)> = None;
    let entries = fs::read_dir(&dir).map_err(|e| format!("state 読取失敗 {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("state エントリ読取失敗: {e}"))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
            best = Some((mtime, stem));
        }
    }
    Ok(best.map(|(_, s)| s))
}
