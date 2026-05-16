//! `harness find-symbol <query> [--lang ...] [--daemon-port <port>] [--use-daemon]` ハンドラ。
//!
//! `--lang auto`: `detect_lang_from_qname(query).or(root_lang(root))`。
//! `--daemon-port <port>` で固定 port、`--use-daemon` で auto-spawn (~/.cache/.../*.port)。

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::ckg::lsp::{find_symbol_for_lang, lsp_server_cmd, Lang, SymbolInfo};
use crate::lsp_daemon::{DaemonClient, SymbolPayload};

/// 優先順: daemon_port > use_daemon > 直接 LSP spawn。
pub fn cmd_find_symbol(
    query: &str,
    kind: Option<&str>,
    root: Option<&str>,
    format: &str,
    lang_arg: &str,
    daemon_port: Option<u16>,
    use_daemon: bool,
) -> Result<(), String> {
    let root_path: PathBuf = match root {
        Some(r) => PathBuf::from(r),
        None => std::env::current_dir().map_err(|e| format!("cwd: {e}"))?,
    };
    let lang_lazy = || resolve_lang(lang_arg, query, &root_path);
    let syms = if let Some(mut c) = open_client(daemon_port, use_daemon, &root_path, &lang_lazy)? {
        let p = c.find_symbol(query, &root_path, kind, Duration::from_secs(60))?;
        p.into_iter().map(payload_to_info).collect()
    } else {
        let lang = lang_lazy()?;
        ensure_server_available(lang)?;
        let timeout = match lang { Lang::Ts => Duration::from_secs(60), _ => Duration::from_secs(30) };
        find_symbol_for_lang(lang, &root_path, query, kind, timeout)?
    };
    match format {
        "json" => print_json(&syms)?,
        "text" => print_text(&syms),
        other => return Err(format!("unknown format: {other} (text|json)")),
    }
    Ok(())
}

/// `daemon_port` > `use_daemon` > None の優先順で `DaemonClient` を返す。
pub(crate) fn open_client<F>(
    daemon_port: Option<u16>,
    use_daemon: bool,
    root: &Path,
    lang_lazy: &F,
) -> Result<Option<DaemonClient>, String>
where
    F: Fn() -> Result<Lang, String>,
{
    if let Some(port) = daemon_port {
        return Ok(Some(DaemonClient::connect(port)?));
    }
    if use_daemon {
        let lang = lang_lazy()?;
        return Ok(Some(DaemonClient::connect_or_spawn(
            lang,
            root,
            Duration::from_secs(30),
        )?));
    }
    Ok(None)
}

fn payload_to_info(p: SymbolPayload) -> SymbolInfo {
    SymbolInfo { name: p.name, kind: p.kind, file: p.file, line: p.line, col: p.col }
}

/// `--lang` 引数を `Lang` に解決する。"auto" は qname → root の順で推定。
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
            Err("言語推定に失敗しました。--lang <rust|ts|py|go> を明示してください".to_string())
        }
        other => Err(format!("unknown --lang: {other} (auto|rust|ts|py|go)")),
    }
}

/// 指定 Lang の LSP server が PATH 上にあるか確認する。無ければ install 案内エラー。
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
    Err(format!("{cmd} が PATH に見つかりません。{hint} でインストールしてください"))
}

#[cfg(windows)]
fn augment_path_from_npm_prefix() -> bool {
    let out = std::process::Command::new("cmd").args(["/c", "npm", "config", "get", "prefix"]).output().ok();
    let prefix = match out {
        Some(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
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

fn which(cmd: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT".to_string()).split(';').map(|s| s.to_string()).collect()
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
        Err(format!("{cmd} が PATH に見つかりません。`rustup component add rust-analyzer` でインストールしてください"))
    }
}

fn print_text(syms: &[SymbolInfo]) {
    for s in syms {
        println!("{} {} at {}:{}", short_kind(&s.kind), s.name, s.file, s.line);
    }
}

fn short_kind(k: &str) -> &str {
    match k {
        "function" | "method" | "constructor" => "fn",
        "interface" => "trait",
        "module" | "namespace" => "mod",
        "constant" => "const",
        "variable" => "static",
        "enum_member" => "variant",
        other => other,
    }
}

fn print_json(syms: &[SymbolInfo]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(syms).map_err(|e| format!("serialize: {e}"))?;
    println!("{json}");
    Ok(())
}
