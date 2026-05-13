//! `.github/workflows/*.yml` を簡易パースして `run:` 行から CI コマンドを拾う。
//!
//! 完全 YAML パースはしない（行ベース）。複数行 `run: |` ブロックは追わない。

use std::fs;
use std::path::Path;

use super::DetectedProject;

pub fn detect_ci(dir: &Path, d: &mut DetectedProject) {
    let wf_dir = dir.join(".github").join("workflows");
    let Ok(entries) = fs::read_dir(&wf_dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if !is_yaml(&p) {
            continue;
        }
        let text = fs::read_to_string(&p).unwrap_or_default();
        for line in text.lines() {
            if let Some(cmd) = strip_run(line) {
                if cmd.is_empty() {
                    continue;
                }
                let l = cmd.to_lowercase();
                let interesting = [
                    "test", "build", "lint", "cargo", "pytest", "pnpm", "npm", "yarn", "bun",
                    "go ", "mvn", "gradle", "make ", "coverage",
                ];
                if interesting.iter().any(|k| l.contains(k)) {
                    d.ci_run_lines.push(cmd.to_string());
                }
            }
        }
    }
}

fn is_yaml(p: &Path) -> bool {
    matches!(p.extension().and_then(|s| s.to_str()), Some("yml") | Some("yaml"))
}

/// `run: foo bar` から `foo bar` を抜く。`|`/`>` のブロック開始は捨てる（単一行ケースだけ拾う）。
fn strip_run(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let rest = t.strip_prefix("run:")?;
    let rest = rest.trim();
    if rest.is_empty() || rest == "|" || rest == ">" || rest == "|-" || rest == ">-" {
        return None;
    }
    Some(rest)
}
