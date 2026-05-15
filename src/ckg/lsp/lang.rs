//! 多言語 LSP の言語識別ヘルパ。
//!
//! - Lang: 対応言語 enum（現状 Rust / Ts）
//! - detect_lang: 拡張子（.rs / .ts / .tsx）から判定
//! - detect_lang_from_qname: 「::」を含めば Rust、「.」を含めば Ts（曖昧 / 不明は None）
//! - root_lang: workspace ルートの Cargo.toml / package.json から判定
//! - lsp_server_cmd: 各 Lang に対応する subprocess コマンドと args
//!
//! 各関数は副作用無し（root_lang のみファイル存在を見る）。

use std::path::Path;

/// 対応言語。現状は Rust と TypeScript のみ。Python / Go は別バッチ送り。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Ts,
}

/// 拡張子から Lang を推定する。
/// - .rs               → Rust
/// - .ts / .tsx        → Ts
/// - 上記以外 / 拡張子無し → None
pub fn detect_lang(file: &Path) -> Option<Lang> {
    let ext = file.extension().and_then(|e| e.to_str())?.to_ascii_lowercase();
    match ext.as_str() {
        "rs" => Some(Lang::Rust),
        "ts" | "tsx" => Some(Lang::Ts),
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
    }
}

/// qname の表記から Lang を推定する。
/// - 「::」を含む → Rust（User::new、crate::foo::bar 等）
/// - 「.」を含む  → Ts（User.create、mod.foo 等）
/// - 両方該当 / 両方非該当 → None（呼び側で root_lang 等にフォールバック）
pub fn detect_lang_from_qname(qname: &str) -> Option<Lang> {
    let has_rust = qname.contains("::");
    let has_ts = qname.contains('.');
    match (has_rust, has_ts) {
        (true, false) => Some(Lang::Rust),
        (false, true) => Some(Lang::Ts),
        _ => None,
    }
}

/// workspace ルートのマーカーファイルから Lang を推定する。
/// 両方ある場合は Rust を優先（Crypto プロジェクトのような Rust + node 混在を想定）。
pub fn root_lang(root: &Path) -> Option<Lang> {
    if root.join("Cargo.toml").is_file() {
        return Some(Lang::Rust);
    }
    if root.join("package.json").is_file() {
        return Some(Lang::Ts);
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
        assert_eq!(detect_lang(&PathBuf::from("a.py")), None);
        assert_eq!(detect_lang(&PathBuf::from("noext")), None);
    }

    #[test]
    fn detect_lang_from_qname_rules() {
        assert_eq!(detect_lang_from_qname("User::new"), Some(Lang::Rust));
        assert_eq!(detect_lang_from_qname("crate::foo::bar"), Some(Lang::Rust));
        assert_eq!(detect_lang_from_qname("User.create"), Some(Lang::Ts));
        // 両方該当: 曖昧 → None
        assert_eq!(detect_lang_from_qname("a::b.c"), None);
        // 両方非該当: 単独 ident → None
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
    }
}
