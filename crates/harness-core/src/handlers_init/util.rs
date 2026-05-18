//! `harness init` / `doctor` のヘルパ（パス解決・サマリ表示・shell 実行・PATH 探索）。
//! Windows 専用。

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::detect::DetectedProject;

pub fn resolve_dir(dir: Option<&str>) -> Result<PathBuf, String> {
    let p = match dir {
        Some(s) => PathBuf::from(s),
        None => std::env::current_dir().map_err(|e| format!("cwd 取得失敗: {e}"))?,
    };
    if !p.exists() {
        return Err(format!("ディレクトリが無い: {}", p.display()));
    }
    Ok(p)
}

pub fn resolve_harness_dir(dir: Option<&str>) -> Result<PathBuf, String> {
    if let Some(s) = dir {
        let cand = PathBuf::from(s).join(".harness");
        if cand.exists() {
            return Ok(cand);
        }
        return Err(format!("{} に .harness/ が無い", s));
    }
    let home = crate::paths::harness_home();
    if home.join("workflow.toml").exists() {
        return Ok(home);
    }
    let cand = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join(".harness");
    if cand.exists() {
        return Ok(cand);
    }
    Err("HARNESS_HOME / <dir>/.harness どちらにも workflow.toml が見つからない".into())
}

pub fn print_summary(d: &DetectedProject) {
    println!("--- 検出サマリ ---");
    println!("言語      : {}", d.lang.as_deref().unwrap_or("(未検出)"));
    println!("build     : {}", d.build.as_deref().unwrap_or("(未検出)"));
    println!("check     : {}", d.check.as_deref().unwrap_or("(未検出)"));
    println!("test      : {}", d.test.as_deref().unwrap_or("(未検出)"));
    println!("lint      : {}", d.lint.as_deref().unwrap_or("(未検出)"));
    println!("coverage  : {}", d.coverage.as_deref().unwrap_or("(未検出)"));
    println!("full_suite: {}", d.full_suite.as_deref().unwrap_or("(未検出)"));
    println!("monorepo  : {}", d.monorepo);
    println!("gitleaks  : {}", d.gitleaks_available);
    if !d.ci_run_lines.is_empty() {
        println!("CI run 行 (.github/workflows): {} 件", d.ci_run_lines.len());
        for c in d.ci_run_lines.iter().take(10) {
            println!("  | {c}");
        }
    }
    for n in &d.notes {
        println!("注: {n}");
    }
}

/// シェル経由でコマンド実行（Windows `cmd /C` 経由）。タイムアウト未実装
/// (runtime 側で導入予定)。
pub fn shell_run(cmd: &str, cwd: &Path) -> Result<bool, String> {
    let mut c = Command::new("cmd");
    c.args(["/C", cmd]);
    c.current_dir(cwd);
    let status = c.status().map_err(|e| e.to_string())?;
    Ok(status.success())
}

/// PATH に実行ファイル `name` があるか（Windows は .exe/.bat/.cmd も試す）。
pub fn path_has(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    for dir in std::env::split_paths(&path) {
        for ext in &["", ".exe", ".bat", ".cmd"] {
            let mut p = dir.clone();
            p.push(format!("{name}{ext}"));
            if p.exists() {
                return true;
            }
        }
    }
    false
}
