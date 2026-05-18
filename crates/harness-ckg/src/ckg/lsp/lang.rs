//! 多言語 LSP の言語識別ヘルパ。
//!
//! - Lang: 対応言語 enum（現状 Rust / Ts / Py / Go）
//! - detect_lang: 拡張子（.rs / .ts / .tsx / .py / .go）から判定
//! - detect_lang_from_qname: 「::」のみ含めば Rust。「.」のみは TS/Py/Go 曖昧で None。
//! - root_lang: workspace ルートの Cargo.toml / package.json / pyproject.toml / go.mod 等から判定
//! - lsp_server_cmd: 各 Lang に対応する subprocess コマンドと args
//!
//! 各関数は副作用無し（root_lang のみファイル存在を見る）。

use std::path::Path;

/// 対応言語。現状は Rust / TypeScript / Python / Go。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Ts,
    Py,
    Go,
}

/// 拡張子から Lang を推定する。
/// - .rs               -> Rust
/// - .ts / .tsx        -> Ts
/// - .py               -> Py
/// - .go               -> Go
/// - 上記以外 / 拡張子無し -> None
pub fn detect_lang(file: &Path) -> Option<Lang> {
    let ext = file.extension().and_then(|e| e.to_str())?.to_ascii_lowercase();
    match ext.as_str() {
        "rs" => Some(Lang::Rust),
        "ts" | "tsx" => Some(Lang::Ts),
        "py" => Some(Lang::Py),
        "go" => Some(Lang::Go),
        _ => None,
    }
}

/// Lang に対応する LSP サーバ起動コマンドと args を返す。
/// 戻り値はそのまま std::process::Command::new(cmd).args(args) に流せる。
pub fn lsp_server_cmd(lang: Lang) -> (String, Vec<String>) {
    match lang {
        Lang::Rust => ("rust-analyzer".to_string(), Vec::new()),
        Lang::Ts => (
            "typescript-language-server".to_string(),
            vec!["--stdio".to_string()],
        ),
        Lang::Py => (
            "pyright-langserver".to_string(),
            vec!["--stdio".to_string()],
        ),
        Lang::Go => ("gopls".to_string(), Vec::new()),
    }
}

/// qname の表記から Lang を推定する。
/// - 「::」のみ含む -> Rust（User::new、crate::foo::bar 等）
/// - 「.」のみ含む -> None（TS の `User.create`、Py の `module.Class.method`、
///   Go の `pkg.Func` / `Type.Method` が衝突するので曖昧 -> 呼び側で root_lang() にフォールバック）
/// - 両方該当 / 両方非該当 -> None
pub fn detect_lang_from_qname(qname: &str) -> Option<Lang> {
    let has_rust = qname.contains("::");
    let has_dot = qname.contains('.');
    match (has_rust, has_dot) {
        (true, false) => Some(Lang::Rust),
        _ => None,
    }
}

/// workspace ルートのマーカーファイルから Lang を推定する。
/// 優先順は Rust > Ts > Py > Go（Crypto 等の混在で既存挙動を維持しつつ Go を末尾追加）。
pub fn root_lang(root: &Path) -> Option<Lang> {
    if root.join("Cargo.toml").is_file() {
        return Some(Lang::Rust);
    }
    if root.join("package.json").is_file() {
        return Some(Lang::Ts);
    }
    if root.join("pyproject.toml").is_file()
        || root.join("setup.py").is_file()
        || root.join("requirements.txt").is_file()
    {
        return Some(Lang::Py);
    }
    if root.join("go.mod").is_file() {
        return Some(Lang::Go);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detect_lang_by_ext() {
        assert_eq!(detect_lang(&PathBuf::from("a.rs")), Some(Lang::Rust));
        assert_eq!(detect_lang(&PathBuf::from("a.ts")), Some(Lang::Ts));
        assert_eq!(detect_lang(&PathBuf::from("a.tsx")), Some(Lang::Ts));
        assert_eq!(detect_lang(&PathBuf::from("a.py")), Some(Lang::Py));
        assert_eq!(detect_lang(&PathBuf::from("a.go")), Some(Lang::Go));
        assert_eq!(detect_lang(&PathBuf::from("noext")), None);
    }

    #[test]
    fn detect_lang_from_qname_rules() {
        assert_eq!(detect_lang_from_qname("User::new"), Some(Lang::Rust));
        assert_eq!(detect_lang_from_qname("crate::foo::bar"), Some(Lang::Rust));
        // 「.」のみは TS/Py/Go で曖昧 -> None（root_lang フォールバック前提）
        assert_eq!(detect_lang_from_qname("User.create"), None);
        assert_eq!(detect_lang_from_qname("module.Class.method"), None);
        assert_eq!(detect_lang_from_qname("pkg.Func"), None);
        // 両方該当: 曖昧 -> None
        assert_eq!(detect_lang_from_qname("a::b.c"), None);
        // 両方非該当: 単独 ident -> None
        assert_eq!(detect_lang_from_qname("User"), None);
    }

    #[test]
    fn lsp_server_cmd_shape() {
        assert_eq!(
            lsp_server_cmd(Lang::Rust),
            ("rust-analyzer".to_string(), Vec::<String>::new())
        );
        let (cmd, args) = lsp_server_cmd(Lang::Ts);
        assert_eq!(cmd, "typescript-language-server");
        assert_eq!(args, vec!["--stdio".to_string()]);
        let (cmd, args) = lsp_server_cmd(Lang::Py);
        assert_eq!(cmd, "pyright-langserver");
        assert_eq!(args, vec!["--stdio".to_string()]);
        assert_eq!(
            lsp_server_cmd(Lang::Go),
            ("gopls".to_string(), Vec::<String>::new())
        );
    }

    #[test]
    fn root_lang_python_pyproject() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("pyproject.toml"), "[project]\n").unwrap();
        assert_eq!(root_lang(tmp.path()), Some(Lang::Py));
    }

    #[test]
    fn root_lang_rust_preferred_over_python() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("pyproject.toml"), "[project]\n").unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]\n").unwrap();
        assert_eq!(root_lang(tmp.path()), Some(Lang::Rust));
    }

    #[test]
    fn root_lang_python_via_setup_py() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("setup.py"), "from setuptools import setup\n").unwrap();
        assert_eq!(root_lang(tmp.path()), Some(Lang::Py));
    }

    #[test]
    fn root_lang_python_via_requirements_txt() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("requirements.txt"), "pytest\n").unwrap();
        assert_eq!(root_lang(tmp.path()), Some(Lang::Py));
    }

    #[test]
    fn root_lang_go() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("go.mod"), "module example.com/sample\ngo 1.21\n").unwrap();
        assert_eq!(root_lang(tmp.path()), Some(Lang::Go));
    }

    #[test]
    fn root_lang_rust_preferred_over_go() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("go.mod"), "module x\n").unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]\n").unwrap();
        assert_eq!(root_lang(tmp.path()), Some(Lang::Rust));
    }
}
