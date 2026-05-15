//! CKG layer 3 SQLite schema。
//!
//! 3 テーブル + meta:
//! - `symbols`: 言語横断 symbol テーブル (qname/kind/file/line/col/lang)
//! - `refs`: 参照エッジ (target_qname を source_file:line から参照する)
//! - `call_edges`: 呼び出しエッジ (source_qname → target_qname、callHierarchy 用)
//! - `index_meta`: schema_version 等のメタ情報

use rusqlite::Connection;

/// 現行 schema バージョン。今後 schema 改変時に上げる。
pub const SCHEMA_VERSION: u32 = 1;

const DDL: &str = r#"
CREATE TABLE IF NOT EXISTS symbols (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    qname TEXT NOT NULL,
    kind TEXT NOT NULL,
    file TEXT NOT NULL,
    line INTEGER NOT NULL,
    col INTEGER NOT NULL DEFAULT 0,
    lang TEXT NOT NULL,
    UNIQUE(qname, file, line, lang)
);
CREATE INDEX IF NOT EXISTS idx_symbols_qname ON symbols(qname);
CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file);

CREATE TABLE IF NOT EXISTS refs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    target_qname TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_line INTEGER NOT NULL,
    source_col INTEGER NOT NULL DEFAULT 0,
    lang TEXT NOT NULL,
    UNIQUE(target_qname, source_file, source_line, lang)
);
CREATE INDEX IF NOT EXISTS idx_refs_target ON refs(target_qname);
CREATE INDEX IF NOT EXISTS idx_refs_source ON refs(source_file);

CREATE TABLE IF NOT EXISTS call_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_qname TEXT NOT NULL,
    target_qname TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_line INTEGER NOT NULL,
    lang TEXT NOT NULL,
    UNIQUE(source_qname, target_qname, source_file, source_line, lang)
);
CREATE INDEX IF NOT EXISTS idx_call_source ON call_edges(source_qname);
CREATE INDEX IF NOT EXISTS idx_call_target ON call_edges(target_qname);

CREATE TABLE IF NOT EXISTS index_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

/// schema を適用し、`index_meta` に `schema_version` を書き込む。
///
/// 冪等 ── 既存 DB に対して再実行しても上書きされるのは `schema_version` のみ。
pub fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(DDL).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO index_meta(key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![SCHEMA_VERSION.to_string()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
