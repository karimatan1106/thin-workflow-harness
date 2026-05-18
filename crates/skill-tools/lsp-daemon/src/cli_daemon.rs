//! `harness lsp-daemon` -- foreground daemon + list/stop CLI。
//!
//! Split from cli.rs to keep 200-line cap. Dispatched from cli_dispatch.

use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Subcommand;

use thin_workflow_harness_ckg::ckg::lsp::Lang;
use thin_workflow_harness_ckg::lsp_daemon::{admin, run_daemon, DaemonClient};

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
        /// idle timeout in minutes (0 = disable, default 30)
        #[arg(long, default_value_t = 30)]
        idle_timeout_min: u64,
    },
    /// 起動中の daemon 一覧を表示する (cache_dir 配下の port file 経由)。
    List,
    /// daemon を停止する。--lang (+ optional --root) or --all or --stale のいずれか必須。
    Stop {
        #[arg(long)]
        lang: Option<String>,
        #[arg(long)]
        root: Option<String>,
        /// 全 daemon を停止する。
        #[arg(long)]
        all: bool,
        /// dead な port file のみ削除する (process は触らない)。
        #[arg(long)]
        stale: bool,
    },
    /// daemon に health 問い合わせ。未起動なら auto-spawn して初期 snapshot を返す。
    Health {
        /// target language (rust | ts | py | go)
        #[arg(long)]
        lang: String,
        /// workspace root
        #[arg(long)]
        root: String,
    },
}

/// `harness lsp-daemon <subcmd>` dispatcher.
pub fn dispatch_lsp_daemon(cmd: LspDaemonCmd) -> Result<(), String> {
    match cmd {
        LspDaemonCmd::Serve { lang, root, port, idle_timeout_min } => {
            let lang = parse_lang(&lang)?;
            let root_path = match root {
                Some(r) => PathBuf::from(r),
                None => std::env::current_dir().map_err(|e| format!("cwd: {e}"))?,
            };
            let idle_timeout = Duration::from_secs(idle_timeout_min * 60);
            run_daemon(lang, root_path, port, idle_timeout)
        }
        LspDaemonCmd::List => admin::cmd_list(),
        LspDaemonCmd::Stop { lang, root, all, stale } => {
            if stale {
                return admin::cmd_stop_stale();
            }
            if all {
                return admin::cmd_stop_all();
            }
            match (lang, root) {
                (Some(l), Some(r)) => {
                    let lang_enum = parse_lang(&l)?;
                    let lang_s = lang_to_str(lang_enum);
                    admin::cmd_stop_specific(lang_s, std::path::Path::new(&r))
                }
                (Some(l), None) => {
                    let lang_enum = parse_lang(&l)?;
                    let lang_s = lang_to_str(lang_enum);
                    admin::cmd_stop_by_lang(lang_s)
                }
                _ => Err("--lang, --all, or --stale required".to_string()),
            }
        }
        LspDaemonCmd::Health { lang, root } => cmd_health(&lang, &root),
    }
}

fn cmd_health(lang: &str, root: &str) -> Result<(), String> {
    let lang_enum = parse_lang(lang)?;
    let mut client = DaemonClient::connect_or_spawn(
        lang_enum,
        Path::new(root),
        Duration::from_secs(60),
    )?;
    let h = client.health()?;
    let out = serde_json::json!({
        "status": h.status,
        "lang": h.lang,
        "uptime_ms": h.uptime_ms,
        "queries_handled": h.queries_handled,
        "recent_avg_ms": h.recent_avg_ms,
    });
    let s = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("serialize: {e}"))?;
    println!("{s}");
    Ok(())
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

fn lang_to_str(lang: Lang) -> &'static str {
    match lang {
        Lang::Rust => "rust",
        Lang::Ts => "ts",
        Lang::Py => "py",
        Lang::Go => "go",
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

    #[test]
    fn lang_to_str_roundtrip() {
        assert_eq!(lang_to_str(Lang::Rust), "rust");
        assert_eq!(lang_to_str(Lang::Ts), "ts");
        assert_eq!(lang_to_str(Lang::Py), "py");
        assert_eq!(lang_to_str(Lang::Go), "go");
    }
}
