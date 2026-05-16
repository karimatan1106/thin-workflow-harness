//! `harness lsp-daemon serve` -- foreground daemon launcher.
//!
//! Split from cli.rs to keep 200-line cap. Dispatched from cli_dispatch.

use std::path::PathBuf;

use clap::Subcommand;

use crate::ckg::lsp::Lang;
use crate::lsp_daemon::run_daemon;

#[derive(Subcommand)]
pub enum LspDaemonCmd {
    /// Launch the LSP daemon in foreground (Ctrl-C to stop).
    Serve {
        /// target language (rust | ts | py | go)
        #[arg(long, default_value = "rust")]
        lang: String,
        /// workspace root (defaults to cwd)
        #[arg(long)]
        root: Option<String>,
        /// TCP port (0 = OS-assigned)
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
}

/// `harness lsp-daemon <subcmd>` dispatcher.
pub fn dispatch_lsp_daemon(cmd: LspDaemonCmd) -> Result<(), String> {
    match cmd {
        LspDaemonCmd::Serve { lang, root, port } => {
            let lang = parse_lang(&lang)?;
            let root_path = match root {
                Some(r) => PathBuf::from(r),
                None => std::env::current_dir().map_err(|e| format!("cwd: {e}"))?,
            };
            run_daemon(lang, root_path, port)
        }
    }
}

fn parse_lang(s: &str) -> Result<Lang, String> {
    match s.to_ascii_lowercase().as_str() {
        "rust" => Ok(Lang::Rust),
        "ts" | "typescript" => Ok(Lang::Ts),
        "py" | "python" => Ok(Lang::Py),
        "go" => Ok(Lang::Go),
        other => Err(format!("unknown lang: {other} (rust|ts|py|go)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lang_accepts_aliases() {
        assert!(matches!(parse_lang("rust"), Ok(Lang::Rust)));
        assert!(matches!(parse_lang("typescript"), Ok(Lang::Ts)));
        assert!(matches!(parse_lang("ts"), Ok(Lang::Ts)));
        assert!(matches!(parse_lang("python"), Ok(Lang::Py)));
        assert!(matches!(parse_lang("go"), Ok(Lang::Go)));
        assert!(parse_lang("foo").is_err());
    }
}
