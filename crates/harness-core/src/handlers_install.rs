//! `harness install <git-url> [dest]` ── plugin-repo（`.harness/` を持つ workspace）を
//! git clone し、`.harness/workflow.toml` の存在を検証して HARNESS_HOME を案内する。
//!
//! plugin-repo の規約: リポジトリ root 直下に `.harness/`（workflow.toml + skills/*.md）。
//! clone 後はそのまま `HARNESS_HOME=<dest>/.harness harness start ...` で駆動できる。
//!
//! ネットワーク I/O（git clone）を伴う唯一のハンドラ ── 失敗は理由付きで返す。

use std::path::{Path, PathBuf};
use std::process::Command;

/// `harness install <git-url> [dest]`。
/// dest 省略時は URL から導いた repo 名を CWD 直下に作る。
pub fn cmd_install(git_url: &str, dest: Option<&str>, force: bool) -> Result<(), String> {
    let name = repo_name_from_url(git_url)?;
    let dest_path = match dest {
        Some(d) => PathBuf::from(d),
        None => std::env::current_dir()
            .map_err(|e| format!("CWD 取得失敗: {e}"))?
            .join(&name),
    };

    if dest_path.exists() {
        if !force {
            return Err(format!(
                "宛先 '{}' は既に存在する ── 上書きするなら --force、別名なら dest 引数を指定せよ",
                dest_path.display()
            ));
        }
        std::fs::remove_dir_all(&dest_path)
            .map_err(|e| format!("既存ディレクトリ削除失敗 '{}': {e}", dest_path.display()))?;
    }

    println!("[install] clone {git_url} → {}", dest_path.display());
    run_git_clone(git_url, &dest_path)?;

    let harness_dir = dest_path.join(".harness");
    let wf = harness_dir.join("workflow.toml");
    if !wf.exists() {
        return Err(format!(
            "clone は成功したが '{}' が無い ── これは harness plugin-repo ではない（root 直下に .harness/workflow.toml が必要）",
            wf.display()
        ));
    }

    let skills = harness_dir.join("skills");
    let skill_count = count_skills(&skills);
    println!("[OK] plugin-repo を導入: {}", dest_path.display());
    println!("  workflow.toml: あり");
    println!("  skills: {skill_count} 件");
    println!();
    println!("使い方（この plugin の workflow で駆動する）:");
    println!("  HARNESS_HOME={} harness start \"<intent>\"", harness_dir.display());
    println!("  HARNESS_HOME={} harness run", harness_dir.display());
    Ok(())
}

/// git URL から repo 名を導く（末尾 `.git` と path を剥がす）。
fn repo_name_from_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim_end_matches('/');
    let last = trimmed
        .rsplit(['/', ':'])
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("git URL から repo 名を導けない: {url}"))?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    if name.is_empty() {
        return Err(format!("git URL から repo 名を導けない: {url}"));
    }
    Ok(name.to_string())
}

/// `git clone --depth 1 <url> <dest>` を実行する。
fn run_git_clone(url: &str, dest: &Path) -> Result<(), String> {
    let out = Command::new("git")
        .args(["clone", "--depth", "1", url])
        .arg(dest)
        .output()
        .map_err(|e| format!("git 実行失敗（git は PATH にあるか）: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git clone 失敗（exit {:?}）: {}", out.status.code(), stderr.trim()));
    }
    Ok(())
}

/// skills ディレクトリ内の `.md` ファイル数を数える（無ければ 0）。
fn count_skills(skills: &Path) -> usize {
    std::fs::read_dir(skills)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
                .count()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_name_strips_git_suffix_and_path() {
        assert_eq!(repo_name_from_url("https://github.com/u/harness-plugin-security.git").unwrap(), "harness-plugin-security");
        assert_eq!(repo_name_from_url("https://github.com/u/foo").unwrap(), "foo");
        assert_eq!(repo_name_from_url("git@github.com:u/bar.git").unwrap(), "bar");
        assert_eq!(repo_name_from_url("https://github.com/u/baz/").unwrap(), "baz");
    }

    #[test]
    fn repo_name_rejects_empty() {
        assert!(repo_name_from_url("").is_err());
    }

    #[test]
    fn count_skills_handles_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(count_skills(&dir.path().join("nonexistent")), 0);
    }

    #[test]
    fn count_skills_counts_md_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("01-a.md"), "x").unwrap();
        std::fs::write(dir.path().join("02-b.md"), "x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        assert_eq!(count_skills(dir.path()), 2);
    }

    #[test]
    fn install_rejects_existing_dest_without_force() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("exists");
        std::fs::create_dir(&dest).unwrap();
        let r = cmd_install("https://example.com/u/repo.git", dest.to_str(), false);
        assert!(r.is_err(), "既存 dest は --force なしで拒否されるべき");
        assert!(r.unwrap_err().contains("既に存在"));
    }
}
