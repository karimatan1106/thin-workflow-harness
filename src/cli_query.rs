//! `harness query <subcommand>` ファサード ── CKG layer 2 の 7 primitive をまとめる。
//!
//! 既存トップレベルの `harness outline` / `find-symbol` / `refs` / `callers` /
//! `closure` / `impacted-by` / `tested-by` は alias として完全に維持。
//! 各バリアントは既存 handler をそのまま呼び出すだけ ── 破壊的変更なし。
//! query 配下では `find-symbol` を `symbol` に短縮（find- prefix 不要）。

use clap::Subcommand;

use crate::{
    handlers_closure, handlers_find_symbol, handlers_impacted, handlers_outline, handlers_refs,
    handlers_tested,
};

#[derive(Subcommand)]
pub enum QueryCmd {
    /// 指定ファイルの outline（トップレベル/主要シンボル）を表示する。
    Outline {
        path: String,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// workspace のシンボル検索（旧 find-symbol）。多言語 LSP 対応。
    Symbol {
        query: String,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
        /// 言語を指定する。auto なら qname/root から推定。
        #[arg(long, default_value = "auto")]
        lang: String,
    },
    /// 指定 symbol への参照箇所一覧。多言語 LSP 対応。
    Refs {
        qname: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
        /// 言語を指定する。auto なら qname/root から推定。
        #[arg(long, default_value = "auto")]
        lang: String,
    },
    /// 指定 function の呼び出し元一覧。多言語 LSP 対応。
    Callers {
        qname: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
        /// 言語を指定する。auto なら qname/root から推定。
        #[arg(long, default_value = "auto")]
        lang: String,
    },
    /// refs/callers の transitive 閉包。多言語 LSP 対応。
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
        /// 言語を指定する。auto なら qname/root から推定。
        #[arg(long, default_value = "auto")]
        lang: String,
    },
    /// 変更影響範囲評価。closure direction=in の薄いラッパ。多言語 LSP 対応。
    ImpactedBy {
        qname: String,
        #[arg(long, default_value_t = 3)]
        depth: usize,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
        /// 言語を指定する。auto なら qname/root から推定。
        #[arg(long, default_value = "auto")]
        lang: String,
    },
    /// 指定 symbol をテストしている test 関数一覧。多言語 LSP 対応。
    TestedBy {
        qname: String,
        #[arg(long, default_value_t = 3)]
        depth: usize,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
        /// 言語を指定する。auto なら qname/root から推定。
        #[arg(long, default_value = "auto")]
        lang: String,
    },
}

/// `Command::Query { cmd }` の dispatch ── 各バリアントを既存 handler に振る。
pub fn dispatch_query(cmd: QueryCmd) -> Result<(), String> {
    match cmd {
        QueryCmd::Outline { path, format } => handlers_outline::cmd_outline(&path, &format),
        QueryCmd::Symbol { query, kind, root, format, lang } => handlers_find_symbol::cmd_find_symbol(
            &query, kind.as_deref(), root.as_deref(), &format, &lang, None, false,
        ),
        QueryCmd::Refs { qname, root, format, lang } => {
            handlers_refs::cmd_refs(&qname, root.as_deref(), &format, &lang, None, false)
        }
        QueryCmd::Callers { qname, root, format, lang } => {
            handlers_refs::cmd_callers(&qname, root.as_deref(), &format, &lang, None, false)
        }
        QueryCmd::Closure { qname, depth, direction, root, format, lang } => {
            handlers_closure::cmd_closure(
                &qname,
                depth,
                &direction,
                root.as_deref(),
                &format,
                &lang,
                None,
                false,
            )
        }
        QueryCmd::ImpactedBy { qname, depth, root, format, lang } => {
            handlers_impacted::cmd_impacted_by(&qname, depth, root.as_deref(), &format, &lang, None, false)
        }
        QueryCmd::TestedBy { qname, depth, root, format, lang } => {
            handlers_tested::cmd_tested_by(&qname, depth, root.as_deref(), &format, &lang, None, false)
        }
    }
}
