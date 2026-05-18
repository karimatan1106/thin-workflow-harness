//! harness-lspd CLI ── CKG primitive (find-symbol/refs/callers/closure/impacted-by/
//! tested-by/outline) + `query` ファサード + `lsp-daemon` serve/list/stop/health。
//!
//! 旧 `harness <ckg-cmd>` を `harness-lspd <ckg-cmd>` に分離した。
//! workflow runner 系コマンドは `harness` 側にそのまま残る。

use clap::{Parser, Subcommand};

use crate::{
    cli_daemon, cli_query, handlers_closure, handlers_find_symbol, handlers_impacted,
    handlers_outline, handlers_refs, handlers_tested,
};

#[derive(Parser)]
#[command(
    name = "harness-lspd",
    about = "thin-workflow-harness LSP daemon + CKG primitive CLI"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// 指定ファイルの outline（トップレベル/主要シンボル）を表示する。CKG layer 1。
    Outline {
        path: String,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// workspace のシンボル検索。CKG layer 2 (多言語 LSP)。daemon 経由が既定。
    FindSymbol {
        query: String,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long, default_value = "auto")]
        lang: String,
        #[arg(long)]
        daemon_port: Option<u16>,
    },
    /// CKG layer 2.5 ── 永続 LSP daemon (foreground PoC)。
    LspDaemon {
        #[command(subcommand)]
        cmd: crate::cli_daemon::LspDaemonCmd,
    },
    /// 指定 symbol への参照箇所一覧。CKG layer 2 (多言語 LSP)。daemon 経由が既定。
    Refs {
        qname: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long, default_value = "auto")]
        lang: String,
        #[arg(long)]
        daemon_port: Option<u16>,
    },
    /// 指定 function の呼び出し元一覧。CKG layer 2 (多言語 LSP)。daemon 経由が既定。
    Callers {
        qname: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long, default_value = "auto")]
        lang: String,
        #[arg(long)]
        daemon_port: Option<u16>,
    },
    /// refs/callers の transitive 閉包。CKG layer 2 (多言語 LSP)。daemon 経由が既定。
    Closure {
        qname: String,
        #[arg(long, default_value_t = 2)]
        depth: usize,
        #[arg(long, default_value = "in")]
        direction: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long, default_value = "auto")]
        lang: String,
        #[arg(long)]
        daemon_port: Option<u16>,
    },
    /// 変更影響範囲評価。closure direction=in の薄いラッパ。CKG layer 2 (多言語 LSP)。daemon 経由が既定。
    ImpactedBy {
        qname: String,
        #[arg(long, default_value_t = 3)]
        depth: usize,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long, default_value = "auto")]
        lang: String,
        #[arg(long)]
        daemon_port: Option<u16>,
    },
    /// 指定 symbol をテストしている test 関数一覧。CKG layer 2 (多言語 LSP)。daemon 経由が既定。
    TestedBy {
        qname: String,
        #[arg(long, default_value_t = 3)]
        depth: usize,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long, default_value = "auto")]
        lang: String,
        #[arg(long)]
        daemon_port: Option<u16>,
    },
    /// CKG layer 2 の query primitive ファサード。
    Query {
        #[command(subcommand)]
        cmd: crate::cli_query::QueryCmd,
    },
}

/// CLI エントリ。`main.rs` から呼ばれる。
pub fn run() -> Result<(), String> {
    let cli = Cli::parse();
    dispatch(cli.command)
}

fn dispatch(command: Command) -> Result<(), String> {
    match command {
        Command::Outline { path, format } => handlers_outline::cmd_outline(&path, &format),
        Command::FindSymbol { query, kind, root, format, lang, daemon_port } => {
            handlers_find_symbol::cmd_find_symbol(
                &query,
                kind.as_deref(),
                root.as_deref(),
                &format,
                &lang,
                daemon_port,
            )
        }
        Command::LspDaemon { cmd } => cli_daemon::dispatch_lsp_daemon(cmd),
        Command::Refs { qname, root, format, lang, daemon_port } => {
            handlers_refs::cmd_refs(&qname, root.as_deref(), &format, &lang, daemon_port)
        }
        Command::Callers { qname, root, format, lang, daemon_port } => {
            handlers_refs::cmd_callers(&qname, root.as_deref(), &format, &lang, daemon_port)
        }
        Command::Closure { qname, depth, direction, root, format, lang, daemon_port } => {
            handlers_closure::cmd_closure(
                &qname,
                depth,
                &direction,
                root.as_deref(),
                &format,
                &lang,
                daemon_port,
            )
        }
        Command::ImpactedBy { qname, depth, root, format, lang, daemon_port } => {
            handlers_impacted::cmd_impacted_by(
                &qname,
                depth,
                root.as_deref(),
                &format,
                &lang,
                daemon_port,
            )
        }
        Command::TestedBy { qname, depth, root, format, lang, daemon_port } => {
            handlers_tested::cmd_tested_by(
                &qname,
                depth,
                root.as_deref(),
                &format,
                &lang,
                daemon_port,
            )
        }
        Command::Query { cmd } => cli_query::dispatch_query(cmd),
    }
}
