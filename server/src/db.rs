//! Database schema and connection handling.
//!
//! # Why the schema is a copy
//!
//! The `BaseNote` table below deliberately reproduces Notally's Room schema — same
//! column names, same order, same types. That is not accident. Importing an Android
//! backup is then a row copy (`INSERT INTO BaseNote SELECT * FROM imported.BaseNote`)
//! rather than a translation layer, and the odds of a silent format mismatch drop to
//! roughly zero.
//!
//! **Consequence: do not reorder, rename, retype, or add columns to `BaseNote` without
//! reading `docs/plan.md` §4 first.** A cosmetic refactor here breaks Android import.
//! New columns go in a separate table.
//!
//! # Migration compatibility
//!
//! Notally's migrations 2 through 5 added `color`, `images`, `audios`, and `reminder`
//! one at a time, so an old backup may lack some of them. The import layer detects this
//! with `PRAGMA table_info` and supplies defaults. See `import/android.rs`.

use anyhow::Result;
use rusqlite::Connection;

/// The Notally Room schema, reproduced verbatim.
///
/// Column order matters: the Android import relies on `SELECT *` mapping positionally.
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS BaseNote (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    type      TEXT    NOT NULL,
    folder    TEXT    NOT NULL,
    color     TEXT    NOT NULL,
    title     TEXT    NOT NULL,
    pinned    INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    labels    TEXT    NOT NULL,
    body      TEXT    NOT NULL,
    spans     TEXT    NOT NULL,
    items     TEXT    NOT NULL,
    images    TEXT    NOT NULL,
    audios    TEXT    NOT NULL,
    reminder  TEXT
);

-- Notally's own indices, including its unusual composite one.
CREATE INDEX IF NOT EXISTS BaseNote_index
    ON BaseNote (id, folder, pinned, timestamp, labels);

CREATE TABLE IF NOT EXISTS Label (
    value TEXT PRIMARY KEY
);
"#;

/// Inkwell's own tables. Kept separate from the Notally-compatible block above so that
/// an Android backup can be attached and copied without name collisions.
pub const INKWELL_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS Setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS User (
    id            INTEGER PRIMARY KEY,
    username      TEXT    NOT NULL,
    password_hash TEXT    NOT NULL
);

-- Full-text search over notes. Not present in Notally: its search used LIKE plus
-- post-filtering, which is why it had false positives on label substrings.
CREATE VIRTUAL TABLE IF NOT EXISTS NoteFts USING fts5(
    title, body, items, labels,
    content='BaseNote',
    content_rowid='id'
);
"#;

/// Open a connection and apply the schema.
pub fn open(path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    init(&conn)?;
    Ok(conn)
}

/// Apply both schema blocks to an existing connection.
pub fn init(conn: &Connection) -> Result<()> {
    // Foreign keys are off by default in SQLite and must be enabled per-connection.
    // Notally does not use them, but our own tables may.
    conn.pragma_update(None, "foreign_keys", "ON")?;

    conn.execute_batch(SCHEMA)?;
    conn.execute_batch(INKWELL_SCHEMA)?;
    Ok(())
}

/// Rebuild the FTS index from `BaseNote`.
///
/// Called after any bulk import. FTS5 is a derived index, so the cheapest correct
/// approach is always to rebuild rather than try to track individual changes.
pub fn rebuild_fts(conn: &Connection) -> Result<()> {
    conn.execute_batch("INSERT INTO NoteFts(NoteFts) VALUES('rebuild');")?;
    Ok(())
}

/// The column names currently present on a table, in declaration order.
///
/// The Android import uses this to find out which of the migration-2-to-5 columns an
/// old backup is missing, and which columns came along in a different order.
pub fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let cols = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(cols)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        init(&c).unwrap();
        c
    }

    #[test]
    fn basenote_column_order_matches_notally() {
        let conn = mem();
        let cols = table_columns(&conn, "BaseNote").unwrap();
        assert_eq!(
            cols,
            vec![
                "id", "type", "folder", "color", "title", "pinned", "timestamp",
                "labels", "body", "spans", "items", "images", "audios", "reminder",
            ],
            "column order is part of the Android import contract"
        );
    }

    #[test]
    fn schema_is_idempotent() {
        let c = mem();
        init(&c).expect("re-running init must be safe");
        init(&c).expect("re-running init must be safe");
    }

    #[test]
    fn reminder_column_is_nullable() {
        let conn = mem();
        conn.execute(
            "INSERT INTO BaseNote (type, folder, color, title, pinned, timestamp,
             labels, body, spans, items, images, audios)
             VALUES ('NOTE','NOTES','DEFAULT','',0,0,'[]','','[]','[]','[]','[]')",
            [],
        )
        .unwrap();
        let reminder: Option<String> = conn
            .query_row("SELECT reminder FROM BaseNote WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(reminder, None, "reminder must accept NULL for legacy notes");
    }

    #[test]
    fn fts_rebuild_indexes_existing_rows() {
        let conn = mem();
        conn.execute(
            "INSERT INTO BaseNote (type, folder, color, title, pinned, timestamp,
             labels, body, spans, items, images, audios)
             VALUES ('NOTE','NOTES','DEFAULT','kettle',0,0,'[]','boil water','[]','[]','[]','[]')",
            [],
        )
        .unwrap();
        rebuild_fts(&conn).unwrap();
        let hits: i64 = conn
            .query_row(
                "SELECT count(*) FROM NoteFts WHERE NoteFts MATCH 'kettle'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1);
    }
}
