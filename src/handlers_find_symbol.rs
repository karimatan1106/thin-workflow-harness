//! `harness find-symbol <query> [--lang auto|rust|ts|py|go]` ハンドラ。
//!
//! - `--lang auto` 既定: `detect_lang_from_qname(query).or(root_lang(root))`
//! - `--lang rust|ts|py|go` 明示: その Lang で固定（`python` は `py`、`typescript` は `ts` の alias）
//! - 推定不能なら「--lang を明示してください」エラー
//!
//! LSP server が PATH に無い時は「インストールしてください」エラー。
//! TypeScript の場合 `typescript-language-server --stdio` を spawn する。
//!
//! 200 行制約のため lang 解決と server 確認は別関数に分ける。

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::ckg::lsp::{find_symbol_for_lang, lsp_server_cmd, Lang, SymbolInfo};

/// CLI ハンドラ本体。
pub fn cmd_find_symbol(
    query: &str,
    kind: Option<&str>,
    root: Option<&str>,
    format: &str,
    lang_arg: &str,
) -> Result<(), String> {
    let root_path: PathBuf = match root {
        Some(r) => PathBuf::from(r),
        None => std::env::current_dir().map_err(|e| format!("cwd: {e}"))?,
    };
    let lang = resolve_lang(lang_arg, query, &root_path)?;
    ensure_server_available(lang)?;
    // TS は tsserver の project ロードが遅い workspace（FundingRate/frontend 等）で
    // 30s では足りない場合がある。lang 毎に既定 timeout を変える。
    let timeout = match lang {
        Lang::Ts => Duration::from_secs(60),
        _ => Duration::from_secs(30),
    };
    let syms = find_symbol_for_lang(lang, &root_path, query, kind, timeout)?;
    match format {
        "json" => print_json(&syms)?,
        "text" => print_text(&syms),
        other => return Err(format!("unknown format: {other} (text|json)")),
    }
    Ok(())
}

/// `--lang` 引数を `Lang` に解決する。
/// - "auto": qname → root の順で推定。決まらなければエラー。
/// - "rust" / "ts" / "py" / "go": 明示固定（py は python、ts は typescript alias 受付）。
/// - その他: エラー。
pub fn resolve_lang(lang_arg: &str, qname: &str, root: &Path) -> Result<Lang, String> {
    match lang_arg.to_ascii_lowercase().as_str() {
        "rust" => Ok(Lang::Rust),
        "ts" | "typescript" => Ok(Lang::Ts),
        "py" | "python" => Ok(Lang::Py),
        "go" => Ok(Lang::Go),
        "auto" => {
            if let Some(l) = crate::ckg::lsp::detect_lang_from_qname(qname) {
                return Ok(l);
            }
            if let Some(l) = crate::ckg::lsp::root_lang(root) {
                return Ok(l);
            }
            Err(
                "言語推定に失敗しました。--lang <rust|ts|py|go> を明示してください"
                    .to_string(),
            )
        }
        other => Err(format!("unknown --lang: {other} (auto|rust|ts|py|go)")),
    }
}

/// 指定 Lang の LSP server が PATH 上にあるか確認する。無ければ install 案内エラー。
///
/// Windows では npm global install の `.cmd` shim が `npm config get prefix` の直下に
/// 置かれることが多く、その prefix が %PATH% に入っていない環境がある。
/// TS/Py（typescript-language-server / pyright）が直接見つからない場合、npm prefix を
/// 拾って一時的に PATH 先頭に append し、再探索する。
pub fn ensure_server_available(lang: Lang) -> Result<(), String> {
    let (cmd, _args) = lsp_server_cmd(lang);
    if which(&cmd).is_some() {
        return Ok(());
    }
    #[cfg(windows)]
    {
        if matches!(lang, Lang::Ts | Lang::Py) && augment_path_from_npm_prefix() && which(&cmd).is_some() {
            return Ok(());
        }
    }
    let hint = match lang {
        Lang::Rust => "`rustup component add rust-analyzer`",
        Lang::Ts => "`npm i -g typescript-language-server typescript`",
        Lang::Py => "`pip install pyright` または `npm i -g pyright`",
        Lang::Go => "`go install golang.org/x/tools/gopls@latest`",
    };
    Err(format!(
        "{cmd} が PATH に見つかりません。{hint} でインストールしてください"
    ))
}

/// Windows のみ: `npm config get prefix` を呼んで PATH 先頭に append する。
/// 既に append 済み or prefix 取得失敗時は何もしない。成功時 true。
#[cfg(windows)]
fn augment_path_from_npm_prefix() -> bool {
    let out = std::process::Command::new("cmd")
        .args(["/c", "npm", "config", "get", "prefix"])
        .output()
        .ok();
    let prefix = match out {
        Some(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        _ => return false,
    };
    if prefix.is_empty() {
        return false;
    }
    let cur = std::env::var("PATH").unwrap_or_default();
    if cur.split(';').any(|p| p.eq_ignore_ascii_case(&prefix)) {
        return true;
    }
    std::env::set_var("PATH", format!("{prefix};{cur}"));
    true
}

/// rust-analyzer / typescript-language-server を PATH から探す（粗いが pragmatic）。
fn which(cmd: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".to_string())
            .split(';')
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path_var) {
        for ext in &exts {
            let candidate = dir.join(format!("{cmd}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// 旧 API 互換 ── rust-analyzer を PATH から探す（cli_query_facade 互換用）。
pub fn resolve_server_cmd() -> Result<String, String> {
    let cmd = "rust-analyzer";
    if which(cmd).is_some() {
        Ok(cmd.to_string())
    } else {
        Err(format!(
            "{cmd} が PATH に見つかりません。`rustup component add rust-analyzer` でインストールしてください"
        ))
    }
}

fn print_text(syms: &[SymbolInfo]) {
    for s in syms {
        println!("{} {} at {}:{}", short_kind(&s.kind), s.name, s.file, s.line);
    }
}

/// `function` → `fn` 等、表示用に短縮。
fn short_kind(k: &str) -> &str {
    match k {
        "function" => "fn",
        "method" => "fn",
        "constructor" => "fn",
        "struct" => "struct",
        "enum" => "enum",
        "interface" => "trait",
        "module" => "mod",
        "namespace" => "mod",
        "constant" => "const",
        "variable" => "static",
        "field" => "field",
        "enum_member" => "variant",
        other => other,
    }
}

fn print_json(syms: &[SymbolInfo]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(syms).map_err(|e| format!("serialize: {e}"))?;
    println!("{json}");
    Ok(())
}
