//! TypeScript LSP bootstrap ── `textDocument/didOpen` を 1 件流して tsserver の
//! project ロードを発火させるヘルパ。tsls は document-driven で、`workspace/symbol`
//! 単発では "No Project" になるため、tsconfig include 配下（src/lib/app 優先）の
//! `.ts`/`.tsx` を 1 件 didOpen してから symbol query に進む。
//! SKIP_DIRS は node_modules / dist / build / .git / out / .next / target 等 bulky を除外。

use std::path::{Path, PathBuf};

use serde_json::json;

use super::client::LspClient;
use super::query::path_to_file_uri;

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    ".git",
    "out",
    ".next",
    "target",
    ".turbo",
    ".cache",
    "coverage",
];

/// tsserver に 1 件 didOpen を送り project をロードさせる。
///
/// - 適当な `.ts` / `.tsx` ファイルを workspace から探す（最初の 1 件で十分）
/// - 内容を読んで `textDocument/didOpen` を notify する
/// - tsserver の project ロード待機として 800ms スリープ
///
/// 失敗（.ts が見つからない／読めない）でも soft fail にすべきだが、ここでは
/// 呼び側が「TS workspace のはず」と判定済みなので Err を返す。
pub fn warm_up_ts_workspace(client: &mut LspClient, root: &Path) -> Result<(), String> {
    let ts_file = match find_first_ts_file(root) {
        Some(p) => p,
        None => return Err(format!("no .ts/.tsx file under {}", root.display())),
    };
    let content = std::fs::read_to_string(&ts_file)
        .map_err(|e| format!("read {}: {}", ts_file.display(), e))?;
    let uri = path_to_file_uri(&ts_file)?;
    let lang_id = match ts_file
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("tsx") => "typescriptreact",
        _ => "typescript",
    };
    let params = json!({
        "textDocument": {
            "uri": uri,
            "languageId": lang_id,
            "version": 1,
            "text": content,
        }
    });
    client.notify("textDocument/didOpen", params)?;
    // tsserver の project ロードは workspace 規模に応じて時間が伸びる。fixture では
    // 800ms で十分だが、実プロジェクト（FundingRate/frontend クラス）では
    // 2000ms 程度ないと workspace/symbol が空配列で返り続ける。
    std::thread::sleep(std::time::Duration::from_millis(2000));
    Ok(())
}

/// root 配下を BFS で歩き、最初に見つけた `.ts` / `.tsx` を返す。
/// SKIP_DIRS は降りない。
///
/// `src/` / `lib/` / `app/` のような典型的なプロジェクトソース配下に `.ts` がある場合は
/// そこを優先的に探す（tsconfig.json の `include` がそれらを指す前提）。これにより、
/// docs/ や scripts/ 配下の独立した `.ts` を拾って tsserver の project ロードが
/// 起きない問題を避けられる。
pub(super) fn find_first_ts_file(root: &Path) -> Option<PathBuf> {
    // 優先サブディレクトリ: tsconfig include の典型値
    for sub in &["src", "lib", "app"] {
        let p = root.join(sub);
        if p.is_dir() {
            if let Some(f) = bfs_first_ts(&p) {
                return Some(f);
            }
        }
    }
    bfs_first_ts(root)
}

/// BFS で root 配下の最初の `.ts`/`.tsx` を返す。SKIP_DIRS / dotfile は降りない。
fn bfs_first_ts(root: &Path) -> Option<PathBuf> {
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let mut subdirs: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let ft = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if ft.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let low = ext.to_ascii_lowercase();
                    if low == "ts" || low == "tsx" {
                        // .d.ts は declaration only で project ロードのトリガに弱い → skip
                        if !path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.ends_with(".d.ts"))
                            .unwrap_or(false)
                        {
                            return Some(path);
                        }
                    }
                }
            } else if ft.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if SKIP_DIRS.contains(&name) {
                    continue;
                }
                if name.starts_with('.') {
                    continue;
                }
                subdirs.push(path);
            }
        }
        // stack に push する順は逆順で、結果として shallow first 寄りになる
        for d in subdirs.into_iter().rev() {
            stack.push(d);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_first_ts_file_skips_node_modules() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("node_modules/foo")).unwrap();
        std::fs::write(tmp.path().join("node_modules/foo/x.ts"), "").unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/a.ts"), "export const x = 1;\n").unwrap();
        let got = find_first_ts_file(tmp.path()).expect("found");
        assert!(got.ends_with("a.ts"), "got {got:?}");
    }

    #[test]
    fn find_first_ts_file_skips_d_ts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("globals.d.ts"), "").unwrap();
        std::fs::write(tmp.path().join("real.ts"), "export const x = 1;\n").unwrap();
        let got = find_first_ts_file(tmp.path()).expect("found");
        assert!(got.ends_with("real.ts"), "got {got:?}");
    }

    #[test]
    fn find_first_ts_file_none_when_no_ts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("a.rs"), "").unwrap();
        assert!(find_first_ts_file(tmp.path()).is_none());
    }

    #[test]
    fn find_first_ts_file_finds_tsx() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("a.tsx"), "export const x = 1;\n").unwrap();
        let got = find_first_ts_file(tmp.path()).expect("found");
        assert!(got.ends_with("a.tsx"), "got {got:?}");
    }

    /// root 直下と src/ の両方に .ts がある場合は src/ を優先する。
    /// tsconfig.json で include:["src"] な vite/next 構成では、root 直下の独立 .ts
    /// （eslint config 等）を拾うと tsserver project がロードされず symbol が出ない。
    #[test]
    fn find_first_ts_file_prefers_src_over_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("eslint.config.ts"), "").unwrap();
        std::fs::create_dir_all(tmp.path().join("src/components")).unwrap();
        std::fs::write(
            tmp.path().join("src/components/Foo.tsx"),
            "export const Foo = () => null;\n",
        )
        .unwrap();
        let got = find_first_ts_file(tmp.path()).expect("found");
        let s = got.to_string_lossy().to_string();
        assert!(s.contains("src"), "want src/ but got {s}");
    }
}
