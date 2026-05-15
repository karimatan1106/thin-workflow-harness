//! `harness index <subcommand>` ── CKG layer 3 (SCIP + SQLite) 操作。
//!
//! Phase A1 ではスキャフォールド: `init` で `.harness/index.db` を新規作成 + migrate。
//! Phase A2 以降に SCIP loader / build / stats を追加予定。

use clap::Subcommand;

use crate::ckg::index::IndexDb;

#[derive(Subcommand)]
pub enum IndexCmd {
    /// `.harness/index.db` を新規作成して migrate を実行する。
    Init {
        /// DB 出力先（既定: .harness/index.db）。
        #[arg(long)]
        out: Option<String>,
    },
}

/// `Command::Index { cmd }` の dispatch。
pub fn dispatch_index(cmd: IndexCmd) -> Result<(), String> {
    match cmd {
        IndexCmd::Init { out } => {
            let path = out.unwrap_or_else(|| ".harness/index.db".to_string());
            let _ = IndexDb::open(std::path::Path::new(&path))?;
            println!("CKG index initialized at {path}");
            Ok(())
        }
    }
}
