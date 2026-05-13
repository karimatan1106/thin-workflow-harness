//! 言語別の build/test/lint/coverage コマンド推定。
//!
//! `Cargo.toml`/`package.json`/`pyproject.toml`/`go.mod`/`pom.xml`/`build.gradle`/`Makefile`
//! ごとに静的なヒューリスティックでコマンドを埋める。検出失敗は呼出元で
//! プレースホルダ (`false # ...`) に置換される。

use std::fs;
use std::path::Path;

use super::DetectedProject;

pub fn detect_rust(dir: &Path, cargo: &Path, d: &mut DetectedProject) {
    d.lang = Some("rust".into());
    let text = fs::read_to_string(cargo).unwrap_or_default();
    let workspace = text.contains("[workspace]");
    d.monorepo = workspace;
    let suffix = if workspace { " --workspace" } else { "" };
    d.build = Some(format!("cargo build{suffix}"));
    d.check = Some(format!("cargo check{suffix}"));
    let nextest = dir.join("target").join("nextest").exists() || text.contains("cargo-nextest");
    d.test = Some(if nextest {
        "cargo nextest run".into()
    } else {
        "cargo test".into()
    });
    d.lint = Some("cargo clippy --all-targets -- -D warnings".into());
    d.full_suite = d.test.clone();
}

pub fn detect_node(dir: &Path, pkg_path: &Path, d: &mut DetectedProject) {
    d.lang = Some("node".into());
    // package-lock.json 専用の分岐は不要（fallback も "npm"）
    let pm = if dir.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if dir.join("yarn.lock").exists() {
        "yarn"
    } else if dir.join("bun.lockb").exists() {
        "bun"
    } else {
        "npm"
    };
    let monorepo = dir.join("pnpm-workspace.yaml").exists()
        || dir.join("nx.json").exists()
        || dir.join("turbo.json").exists();
    d.monorepo = monorepo;
    if monorepo {
        d.notes.push("monorepo マーカ検出 (pnpm-workspace.yaml / nx.json / turbo.json)".into());
    }
    let pkg_text = fs::read_to_string(pkg_path).unwrap_or_default();
    let scripts = extract_scripts(&pkg_text);
    let mk = |script: &str| format!("{pm} run {script}");
    if scripts.iter().any(|s| s == "test") {
        d.test = Some(mk("test"));
    }
    if scripts.iter().any(|s| s == "build") {
        d.build = Some(mk("build"));
    }
    if let Some(s) = scripts.iter().find(|s| *s == "lint" || *s == "typecheck") {
        d.lint = Some(mk(s));
    }
    if let Some(s) = scripts.iter().find(|s| s.contains("coverage")) {
        d.coverage = Some(mk(s));
    }
    d.full_suite = d.test.clone();
}

/// `package.json` 中の `"scripts": { ... }` の key を粗く抽出する。完全な JSON パース
/// を避け、依存削減のため文字列ベース。
fn extract_scripts(pkg_text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(idx) = pkg_text.find("\"scripts\"") else { return out };
    let rest = &pkg_text[idx..];
    let Some(open) = rest.find('{') else { return out };
    let mut depth = 0i32;
    let mut body = String::new();
    for c in rest[open..].chars() {
        match c {
            '{' => {
                depth += 1;
                if depth > 1 {
                    body.push(c);
                }
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                body.push(c);
            }
            _ => body.push(c),
        }
    }
    let mut in_key = false;
    let mut key = String::new();
    let mut after_colon = false;
    for c in body.chars() {
        match c {
            '"' if !after_colon => {
                if in_key {
                    if !key.is_empty() {
                        out.push(key.clone());
                    }
                    key.clear();
                    in_key = false;
                } else {
                    in_key = true;
                }
            }
            _ if in_key => key.push(c),
            ':' => after_colon = true,
            ',' => after_colon = false,
            _ => {}
        }
    }
    out
}

pub fn detect_python(d: &mut DetectedProject) {
    d.lang = Some("python".into());
    d.test = Some("pytest -q --tb=short".into());
    d.lint = Some("ruff check .".into());
    d.full_suite = d.test.clone();
}

pub fn detect_go(d: &mut DetectedProject) {
    d.lang = Some("go".into());
    d.build = Some("go build ./...".into());
    d.test = Some("go test ./...".into());
    d.lint = Some("go vet ./...".into());
    d.full_suite = d.test.clone();
}

pub fn detect_maven(d: &mut DetectedProject) {
    d.lang = Some("jvm-maven".into());
    d.build = Some("mvn compile".into());
    d.test = Some("mvn test".into());
    d.full_suite = d.test.clone();
}

pub fn detect_gradle(d: &mut DetectedProject) {
    d.lang = Some("jvm-gradle".into());
    d.build = Some("gradle build -x test".into());
    d.test = Some("gradle test".into());
    d.full_suite = d.test.clone();
}

pub fn detect_makefile(makefile: &Path, d: &mut DetectedProject) {
    let text = fs::read_to_string(makefile).unwrap_or_default();
    for line in text.lines() {
        let trimmed = line.trim_start();
        for target in ["test", "build", "lint", "check", "coverage"] {
            let prefix = format!("{target}:");
            if trimmed.starts_with(&prefix) {
                let slot = match target {
                    "test" => &mut d.test,
                    "build" => &mut d.build,
                    "lint" => &mut d.lint,
                    "check" => &mut d.check,
                    "coverage" => &mut d.coverage,
                    _ => continue,
                };
                if slot.is_none() {
                    *slot = Some(format!("make {target}"));
                }
            }
        }
    }
}
