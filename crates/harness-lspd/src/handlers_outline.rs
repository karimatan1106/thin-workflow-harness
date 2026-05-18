//! `harness outline <file>` ハンドラ。
//!
//! ckg::outline_file を呼び、text/json で stdout に表示する。

use std::path::Path;

use thin_workflow_harness_ckg::ckg::{outline_file, Symbol};

/// 出力フォーマット。
pub enum OutlineFormat {
    Text,
    Json,
}

impl OutlineFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "text" => Ok(OutlineFormat::Text),
            "json" => Ok(OutlineFormat::Json),
            other => Err(format!("unknown format: {other} (text|json)")),
        }
    }
}

/// CLI ハンドラ本体。
pub fn cmd_outline(path: &str, format: &str) -> Result<(), String> {
    let fmt = OutlineFormat::parse(format)?;
    let syms = outline_file(Path::new(path))?;
    match fmt {
        OutlineFormat::Text => print_text(&syms),
        OutlineFormat::Json => print_json(&syms)?,
    }
    Ok(())
}

fn print_text(syms: &[Symbol]) {
    for sym in syms {
        print_text_line(sym, 0);
    }
}

fn print_text_line(sym: &Symbol, depth: usize) {
    let indent = "  ".repeat(depth);
    println!(
        "{indent}{sig} at {start}-{end}",
        sig = sym.signature,
        start = sym.start_line,
        end = sym.end_line,
    );
    for ch in &sym.children {
        print_text_line(ch, depth + 1);
    }
}

fn print_json(syms: &[Symbol]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(syms)
        .map_err(|e| format!("serialize: {e}"))?;
    println!("{json}");
    Ok(())
}
