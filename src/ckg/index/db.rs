//! CKG layer 3 ── `IndexDb` ファサード。
//!
//! - `open(path)`: 親ディレクトリ作成 → 接続 → migrate
//! - `open_in_memory()`: テスト用
//! - `insert_symbol` / `find_symbol` / `count_symbols` / `schema_version`
//!
//! エラーは全て `String`（rusqlite::Error は `.to_string()` で潰す）。

use std::path::Path;

use rusqlite::Connection;

use super::schema::migrate;

/// `symbols` テーブルの 1 行を表す薄い DTO。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRow {
    pub qname: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub lang: String,
}

/// SQLite 接続をラップした index facade。
pub struct IndexDb {
    conn: Connection,
}

impl IndexDb {
    /// 指定 path で DB を開く。親ディレクトリが無ければ作成し、schema を migrate する。
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        migrate(&conn)?;
        Ok(Self { conn })
    }

    /// テスト用 in-memory DB を開く。
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        migrate(&conn)?;
        Ok(Self { conn })
    }

    /// symbol を 1 件 INSERT OR IGNORE で登録する。
    pub fn insert_symbol(
        &self,
        qname: &str,
        kind: &str,
        file: &str,
        line: usize,
        col: usize,
        lang: &str,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO symbols(qname, kind, file, line, col, lang)                  VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![qname, kind, file, line as i64, col as i64, lang],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// qname 完全一致で symbol を引く（複数返る可能性あり）。
    pub fn find_symbol(&self, qname: &str) -> Result<Vec<SymbolRow>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT qname, kind, file, line, col, lang FROM symbols                  WHERE qname = ?1 ORDER BY file, line",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![qname], |row| {
                Ok(SymbolRow {
                    qname: row.get(0)?,
                    kind: row.get(1)?,
                    file: row.get(2)?,
                    line: row.get::<_, i64>(3)? as usize,
                    col: row.get::<_, i64>(4)? as usize,
                    lang: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// symbols テーブルの全行数を返す（テスト/診断用）。
    pub fn count_symbols(&self) -> Result<usize, String> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        Ok(n as usize)
    }

    /// `index_meta` の schema_version を返す。未設定なら None。
    pub fn schema_version(&self) -> Result<Option<u32>, String> {
        let res: rusqlite::Result<String> = self.conn.query_row(
            "SELECT value FROM index_meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        );
        match res {
            Ok(v) => v.parse::<u32>().map(Some).map_err(|e| e.to_string()),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn open_in_memory_creates_schema() {
        let db = IndexDb::open_in_memory().expect("open in mem");
        assert_eq!(db.schema_version().expect("ver"), Some(1));
        assert_eq!(db.count_symbols().expect("count"), 0);
    }

    #[test]
    fn insert_and_find_symbol() {
        let db = IndexDb::open_in_memory().unwrap();
        db.insert_symbol("foo::bar", "function", "src/foo.rs", 10, 4, "rust").unwrap();
        let rows = db.find_symbol("foo::bar").unwrap();
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.qname, "foo::bar");
        assert_eq!(r.kind, "function");
        assert_eq!(r.file, "src/foo.rs");
        assert_eq!(r.line, 10);
        assert_eq!(r.col, 4);
        assert_eq!(r.lang, "rust");
    }

    #[test]
    fn insert_or_ignore_duplicate() {
        let db = IndexDb::open_in_memory().unwrap();
        db.insert_symbol("a::b", "function", "f.rs", 1, 0, "rust").unwrap();
        db.insert_symbol("a::b", "function", "f.rs", 1, 0, "rust").unwrap();
        assert_eq!(db.count_symbols().unwrap(), 1);
    }

    #[test]
    fn find_symbol_returns_multiple() {
        let db = IndexDb::open_in_memory().unwrap();
        db.insert_symbol("dup", "function", "a.rs", 1, 0, "rust").unwrap();
        db.insert_symbol("dup", "function", "b.rs", 2, 0, "rust").unwrap();
        db.insert_symbol("dup", "function", "c.rs", 3, 0, "rust").unwrap();
        let rows = db.find_symbol("dup").unwrap();
        assert_eq!(rows.len(), 3);
        let files: Vec<&str> = rows.iter().map(|r| r.file.as_str()).collect();
        assert_eq!(files, vec!["a.rs", "b.rs", "c.rs"]);
    }

    #[test]
    fn open_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sub").join("nested").join("index.db");
        assert!(!path.parent().unwrap().exists());
        let db = IndexDb::open(&path).expect("open w/ parent");
        assert_eq!(db.schema_version().unwrap(), Some(1));
        assert!(path.exists());
    }

    #[test]
    fn find_symbol_unknown_returns_empty() {
        let db = IndexDb::open_in_memory().unwrap();
        let rows = db.find_symbol("nope").unwrap();
        assert!(rows.is_empty());
    }
}
