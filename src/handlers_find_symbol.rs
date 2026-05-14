//! `harness find-symbol <query>` ハンドラ。
//!
//! rust-analyzer を spawn して `workspace/symbol` を 1 回 round-trip させ、
//! 結果を text/json で stdout に出力する。
//! rust-analyzer が PATH に無ければ「インストールしてください」エラー。

use std::path::PathBuf;
use std::time::Duration;

use crate::ckg::lsp::{find_symbol, SymbolInfo};

/// CLI ハンドラ本体。
pub fn cmd_find_symbol(
    query: &str,
    kind: Option<&str>,
    root: Option<&str>,
    format: &str,
) -> Result<(), String> {
    let server_cmd = resolve_server_cmd()?;
    let root_path: PathBuf = match root {
        Some(r) => PathBuf::from(r),
        None => std::env::current_dir().map_err(|e| format!("cwd: {e}"))?,
    };
    let timeout = Duration::from_secs(30);
    let syms = find_symbol(&server_cmd, &root_path, query, kind, timeout)?;
    match format {
        "json" => print_json(&syms)?,
        "text" => print_text(&syms),
        other => return Err(format!("unknown format: {other} (text|json)")),
    }
    Ok(())
}

/// rust-analyzer を PATH から探す（粗いが pragmatic）。
fn resolve_server_cmd() -> Result<String, String> {
    let cmd = "rust-analyzer";
    if which(cmd).is_some() {
        Ok(cmd.to_string())
    } else {
        Err(format!(
            "{cmd} が PATH に見つかりません。`rustup component add rust-analyzer` でインストールしてください"
        ))
    }
}

/// PATH lookup（最小実装、`which` クレートに頼らない）。
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
