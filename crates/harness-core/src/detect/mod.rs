//! プロジェクト構造検出 ── `harness init` のための軽量スキャナ。
//!
//! `Cargo.toml` / `package.json` / `pyproject.toml` / `go.mod` / `pom.xml` / `build.gradle`
//! / `Makefile` / `.github/workflows/*.yml` を順に当て、build/test/lint/coverage/full
//! スイートのコマンドを推定する。完全 YAML パースはせず、`run:` 行ベースの簡易抽出。

mod ci;
mod lang;

use std::path::{Path, PathBuf};

pub use ci::detect_ci;

/// 検出結果。未検出は None / 空。
#[derive(Debug, Clone, Default)]
pub struct DetectedProject {
    pub lang: Option<String>,
    pub build: Option<String>,
    pub test: Option<String>,
    pub lint: Option<String>,
    pub coverage: Option<String>,
    pub full_suite: Option<String>,
    pub check: Option<String>,
    pub ci_run_lines: Vec<String>,
    pub monorepo: bool,
    pub gitleaks_available: bool,
    pub notes: Vec<String>,
}

/// `<dir>` を走査し検出結果を返す。
pub fn detect(dir: &Path) -> DetectedProject {
    let mut d = DetectedProject::default();
    let cargo = dir.join("Cargo.toml");
    let pkg = dir.join("package.json");
    let pyproject = dir.join("pyproject.toml");
    let setup_py = dir.join("setup.py");
    let go_mod = dir.join("go.mod");
    let pom = dir.join("pom.xml");
    let gradle = dir.join("build.gradle");
    let gradle_kts = dir.join("build.gradle.kts");
    let makefile = dir.join("Makefile");

    if cargo.exists() {
        lang::detect_rust(dir, &cargo, &mut d);
    } else if pkg.exists() {
        lang::detect_node(dir, &pkg, &mut d);
    } else if pyproject.exists() || setup_py.exists() {
        lang::detect_python(&mut d);
    } else if go_mod.exists() {
        lang::detect_go(&mut d);
    } else if pom.exists() {
        lang::detect_maven(&mut d);
    } else if gradle.exists() || gradle_kts.exists() {
        lang::detect_gradle(&mut d);
    }

    if makefile.exists() {
        lang::detect_makefile(&makefile, &mut d);
    }

    detect_ci(dir, &mut d);

    if is_in_path("gitleaks") {
        d.gitleaks_available = true;
    }

    d
}

/// PATH に実行ファイル `name` があるか（Windows は .exe/.bat/.cmd も試す）。
pub fn is_in_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    for dir in std::env::split_paths(&path) {
        for ext in &["", ".exe", ".bat", ".cmd"] {
            let mut p: PathBuf = dir.clone();
            p.push(format!("{name}{ext}"));
            if p.exists() {
                return true;
            }
        }
    }
    false
}
