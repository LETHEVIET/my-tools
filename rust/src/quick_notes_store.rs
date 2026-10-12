//! Local note database, including one-time safe migration from legacy .txt files.
//! SQLite/FTS5 are bundled in the application: no external DB or server.
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub kind: String,
    pub tags: String,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

fn root() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|profile| PathBuf::from(profile).join("MyTools"))
        .ok_or_else(|| "Cannot find local Windows user profile".to_owned())
}

/// Kept for opening the original legacy files in Explorer.
pub fn folder() -> Result<PathBuf, String> {
    Ok(root()?.join("quick-notes"))
}

pub fn data_folder() -> Result<PathBuf, String> {
    root()
}

pub fn database_path() -> Result<PathBuf, String> {
    Ok(root()?.join("my-tools.db"))
}

fn timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn title_of(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Untitled note")
        .chars()
        .take(100)
        .collect()
}

fn connect(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create notes directory: {e}"))?;
    }
    let mut conn =
        Connection::open(path).map_err(|e| format!("Cannot open notes database: {e}"))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| format!("Cannot read notes schema version: {e}"))?;
    if version > 1 {
        return Err(format!(
            "This notes database uses schema version {version}. Update My Tools to open it safely."
        ));
    }
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA foreign_keys=ON;
         CREATE TABLE IF NOT EXISTS notes (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           title TEXT NOT NULL,
           body TEXT NOT NULL,
           kind TEXT NOT NULL DEFAULT 'quick' CHECK(kind IN ('quick','prompt')),
           tags TEXT NOT NULL DEFAULT '',
           pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0,1)),
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL,
           legacy_path TEXT UNIQUE
         );
         CREATE TABLE IF NOT EXISTS app_settings (
           key TEXT PRIMARY KEY, value TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_notes_sort ON notes(pinned DESC, updated_at DESC);
         CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
           title, body, tags, content='notes', content_rowid='id'
         );
         CREATE TRIGGER IF NOT EXISTS notes_insert AFTER INSERT ON notes BEGIN
           INSERT INTO notes_fts(rowid,title,body,tags)
             VALUES(new.id,new.title,new.body,new.tags);
         END;
         CREATE TRIGGER IF NOT EXISTS notes_delete AFTER DELETE ON notes BEGIN
           INSERT INTO notes_fts(notes_fts,rowid,title,body,tags)
             VALUES('delete',old.id,old.title,old.body,old.tags);
         END;
         CREATE TRIGGER IF NOT EXISTS notes_update AFTER UPDATE ON notes BEGIN
           INSERT INTO notes_fts(notes_fts,rowid,title,body,tags)
             VALUES('delete',old.id,old.title,old.body,old.tags);
           INSERT INTO notes_fts(rowid,title,body,tags)
             VALUES(new.id,new.title,new.body,new.tags);
         END;",
    )
    .map_err(|e| format!("Cannot initialize notes database: {e}"))?;
    migrate_legacy(&mut conn, &path.with_file_name("quick-notes"))?;
    if version == 0 {
        conn.pragma_update(None, "user_version", 1)
            .map_err(|e| format!("Could not record database schema version: {e}"))?;
    }
    Ok(conn)
}

/// One transaction imports all old files; the completion marker is committed
/// alongside the notes, so interrupted imports can retry without duplication.
/// The original .txt files are never removed or modified.
fn migrate_legacy(conn: &mut Connection, dir: &Path) -> Result<(), String> {
    let complete: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='legacy_notes_imported'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if complete.is_some() {
        return Ok(());
    }
    let mut files = vec![];
    if dir.is_dir() {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_file()
                && path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|name| name.starts_with("note-") && name.ends_with(".txt"))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let transaction = conn.transaction().map_err(|e| e.to_string())?;
    for path in files {
        // If a file is unreadable, fail without marking migration complete;
        // fix the file and the next app start will retry the whole transaction.
        let text = fs::read_to_string(&path)
            .map_err(|e| format!("Could not import {}: {e}", path.display()))?;
        if text.trim().is_empty() {
            continue;
        }
        let file_time = path
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|age| age.as_millis() as i64)
            .unwrap_or_else(timestamp);
        transaction
            .execute(
                "INSERT OR IGNORE INTO notes(title,body,kind,created_at,updated_at,legacy_path)
             VALUES(?1,?2,'quick',?3,?3,?4)",
                params![
                    title_of(&text),
                    text,
                    file_time,
                    path.to_string_lossy().as_ref()
                ],
            )
            .map_err(|e| format!("Cannot import {}: {e}", path.display()))?;
    }
    transaction
        .execute(
            "INSERT OR REPLACE INTO app_settings(key,value)
         VALUES('legacy_notes_imported','1')",
            [],
        )
        .map_err(|e| e.to_string())?;
    transaction
        .commit()
        .map_err(|e| format!("Cannot complete note migration: {e}"))
}

fn with_db<T>(action: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    action(&connect(&database_path()?)?)
}

pub fn initialize() -> Result<(), String> {
    with_db(|_| Ok(()))
}

pub fn save(text: &str) -> Result<i64, String> {
    save_kind(text, "quick")
}

pub fn save_kind(text: &str, kind: &str) -> Result<i64, String> {
    if text.trim().is_empty() {
        return Err("Type a note before saving.".into());
    }
    if kind != "quick" && kind != "prompt" {
        return Err("Unsupported note type".into());
    }
    with_db(|conn| {
        let now = timestamp();
        conn.execute(
            "INSERT INTO notes(title,body,kind,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?4)",
            params![title_of(text), text, kind, now],
        )
        .map_err(|e| format!("Could not save note: {e}"))?;
        Ok(conn.last_insert_rowid())
    })
}

fn map_note(row: &rusqlite::Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        title: row.get(1)?,
        body: row.get(2)?,
        kind: row.get(3)?,
        tags: row.get(4)?,
        pinned: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

const COLS: &str = "n.id,n.title,n.body,n.kind,n.tags,n.pinned,n.created_at,n.updated_at";

pub fn get(id: i64) -> Result<Option<Note>, String> {
    with_db(|conn| {
        conn.query_row(
            "SELECT id,title,body,kind,tags,pinned,created_at,updated_at FROM notes WHERE id=?1",
            [id],
            map_note,
        )
        .optional()
        .map_err(|e| e.to_string())
    })
}

fn search_query(query: &str) -> String {
    // FTS5 MATCH syntax is not SQL syntax. Treat users' input as literal
    // terms instead of exposing FTS operators/quotes, punctuation safely.
    query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub fn list(query: &str, limit: usize) -> Result<Vec<Note>, String> {
    with_db(|conn| list_conn(conn, query, limit))
}

fn list_conn(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Note>, String> {
    let limit = (limit.min(1000)) as i64;
    let sql = if query.trim().is_empty() {
        format!("SELECT {COLS} FROM notes n ORDER BY n.pinned DESC,n.updated_at DESC,n.id DESC LIMIT ?1")
    } else {
        format!("SELECT {COLS} FROM notes n JOIN notes_fts f ON f.rowid=n.id
                WHERE notes_fts MATCH ?1 ORDER BY n.pinned DESC,n.updated_at DESC,n.id DESC LIMIT ?2")
    };
    let mut statement = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let results = if query.trim().is_empty() {
        statement.query_map([limit], map_note)
    } else {
        statement.query_map(params![search_query(query), limit], map_note)
    }
    .map_err(|e| format!("Could not search notes: {e}"))?;
    results
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| format!("Could not read notes: {e}"))
}

pub fn update(id: i64, title: &str, body: &str, tags: &str) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("Note title cannot be empty".into());
    }
    if body.trim().is_empty() {
        return Err("Note cannot be empty".into());
    }
    with_db(|conn| {
        let changed = conn
            .execute(
                "UPDATE notes SET title=?2,body=?3,tags=?4,updated_at=?5 WHERE id=?1",
                params![id, title.trim(), body, tags.trim(), timestamp()],
            )
            .map_err(|e| format!("Could not edit note: {e}"))?;
        if changed == 0 {
            Err("Note no longer exists".into())
        } else {
            Ok(())
        }
    })
}

pub fn set_pinned(id: i64, pinned: bool) -> Result<(), String> {
    with_db(|conn| {
        let changed = conn
            .execute(
                "UPDATE notes SET pinned=?2 WHERE id=?1",
                params![id, pinned as i32],
            )
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            Err("Note no longer exists".into())
        } else {
            Ok(())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "my-tools-db-{label}-{}-{}",
            std::process::id(),
            timestamp()
        ))
    }
    #[test]
    fn create_search_edit_pin_and_reopen() {
        let dir = path("crud");
        fs::create_dir_all(&dir).unwrap();
        let db = dir.join("my-tools.db");
        let conn = connect(&db).unwrap();
        let now = timestamp();
        conn.execute(
            "INSERT INTO notes(title,body,kind,tags,created_at,updated_at)
          VALUES('Build speed','Rust GPU caching','quick','performance',?1,?1)",
            [now],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        let matches = conn
            .query_row(
                "SELECT count(*) FROM notes_fts WHERE notes_fts MATCH 'caching'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(matches, 1);
        assert_eq!(list_conn(&conn, "caching", 20).unwrap().len(), 1);
        assert_eq!(list_conn(&conn, "performance", 20).unwrap().len(), 1);
        assert!(list_conn(&conn, "missing words", 20).unwrap().is_empty());
        conn.execute("UPDATE notes SET title='Updated',body='vector index',tags='ideas',pinned=1 WHERE id=?1",[id]).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM notes_fts WHERE notes_fts MATCH 'vector'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM notes_fts WHERE notes_fts MATCH 'caching'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(list_conn(&conn, "vector", 20).unwrap()[0].title, "Updated");
        assert_eq!(list_conn(&conn, "ideas", 20).unwrap()[0].id, id);
        assert!(list_conn(&conn, "caching", 20).unwrap().is_empty());
        drop(conn);
        assert!(connect(&db).is_ok());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn imports_once_without_touching_legacy_files() {
        let dir = path("migration");
        let original = dir.join("quick-notes");
        fs::create_dir_all(&original).unwrap();
        let note = original.join("note-001.txt");
        fs::write(&note, "One old note\n你好").unwrap();
        let db = dir.join("my-tools.db");
        let conn = connect(&db).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |row| row.get::<_, i32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM notes", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        drop(conn);
        let conn = connect(&db).unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM notes", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row("SELECT body FROM notes", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "One old note\n你好"
        );
        drop(conn);
        assert_eq!(fs::read_to_string(note).unwrap(), "One old note\n你好");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn invalid_legacy_file_blocks_migration_until_fixed() {
        let dir = path("invalid-legacy");
        let originals = dir.join("quick-notes");
        fs::create_dir_all(&originals).unwrap();
        let good = originals.join("note-001.txt");
        let invalid = originals.join("note-002.txt");
        fs::write(&good, "Important old note").unwrap();
        fs::write(&invalid, [0xff, 0xfe]).unwrap();
        let db = dir.join("my-tools.db");
        assert!(connect(&db).is_err());
        let conn = Connection::open(&db).unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM notes", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
        drop(conn);
        fs::write(&invalid, "Another important note").unwrap();
        let conn = connect(&db).unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM notes", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
        drop(conn);
        assert_eq!(fs::read_to_string(good).unwrap(), "Important old note");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn fts_escapes_search_syntax() {
        assert_eq!(search_query("a \"b"), "\"a\" AND \"\"\"b\"");
    }
}
