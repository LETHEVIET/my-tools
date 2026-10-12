//! Quick Tasks stored alongside notes in My Tools SQLite. No cloud or worker
//! process. Due times are persisted in UTC epoch milliseconds, with local
//! timestamps converted only at the UI boundary.
use chrono::{DateTime, Local, LocalResult, NaiveDateTime, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};

use crate::quick_notes_store;

#[derive(Clone, Debug)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub due_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub notified_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskFilter {
    Today,
    Upcoming,
    Completed,
    All,
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn parse_local_due(text: &str) -> Result<Option<i64>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let parsed = NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M")
        .map_err(|_| "Use YYYY-MM-DD HH:MM (local time), or leave blank.".to_owned())?;
    let local = match Local.from_local_datetime(&parsed) {
        LocalResult::Single(time) => time,
        LocalResult::Ambiguous(_, _) => {
            return Err(
                "That time occurs twice because of a clock change. Choose a different time.".into(),
            )
        }
        LocalResult::None => {
            return Err("That local time does not exist because of a clock change.".into())
        }
    };
    if local.timestamp_millis() <= now_ms() {
        return Err("Choose a future reminder time.".into());
    }
    Ok(Some(local.timestamp_millis()))
}

pub fn local_due_label(due: Option<i64>) -> String {
    match due
        .and_then(DateTime::<chrono::Utc>::from_timestamp_millis)
        .map(|dt| dt.with_timezone(&Local))
    {
        Some(date) => date.format("%Y-%m-%d %H:%M").to_string(),
        None => "No reminder".into(),
    }
}

pub fn tomorrow_morning() -> String {
    let today = Local::now().date_naive();
    let tomorrow = today.succ_opt().unwrap_or(today);
    format!("{} 09:00", tomorrow.format("%Y-%m-%d"))
}

fn open() -> Result<Connection, String> {
    let conn = quick_notes_store::open_db()?;
    init(&conn)?;
    Ok(conn)
}

fn init(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tasks (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           title TEXT NOT NULL CHECK(length(trim(title)) > 0),
           due_at INTEGER,
           notified_at INTEGER,
           completed_at INTEGER,
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_tasks_due
           ON tasks(due_at, id) WHERE completed_at IS NULL AND notified_at IS NULL;
         CREATE INDEX IF NOT EXISTS idx_tasks_recent
           ON tasks(completed_at, due_at, created_at);",
    )
    .map_err(|e| format!("Cannot initialize Quick Tasks: {e}"))
}

fn decode(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        due_at: row.get(2)?,
        completed_at: row.get(3)?,
        notified_at: row.get(4)?,
    })
}

pub fn get(id: i64) -> Result<Option<Task>, String> {
    open()?
        .query_row(
            "SELECT id,title,due_at,completed_at,notified_at FROM tasks WHERE id=?1",
            [id],
            decode,
        )
        .optional()
        .map_err(|e| e.to_string())
}

fn add_with(conn: &Connection, title: &str, due_at: Option<i64>, now: i64) -> Result<i64, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Enter a task first.".into());
    }
    if title.chars().count() > 300 {
        return Err("Task title is too long (300 characters maximum).".into());
    }
    if let Some(due) = due_at {
        if due <= now {
            return Err("Reminder must be in the future.".into());
        }
    }
    conn.execute(
        "INSERT INTO tasks(title,due_at,created_at,updated_at) VALUES(?1,?2,?3,?3)",
        params![title, due_at, now],
    )
    .map_err(|e| format!("Cannot save task: {e}"))?;
    Ok(conn.last_insert_rowid())
}

pub fn add(title: &str, due_at: Option<i64>) -> Result<i64, String> {
    add_with(&open()?, title, due_at, now_ms())
}

fn list_with(
    conn: &Connection,
    filter: TaskFilter,
    search: &str,
    now: i64,
    tomorrow: i64,
) -> Result<Vec<Task>, String> {
    let condition = match filter {
        TaskFilter::Today => "completed_at IS NULL AND due_at IS NOT NULL AND due_at < ?2",
        TaskFilter::Upcoming => "completed_at IS NULL AND (due_at IS NULL OR due_at >= ?2)",
        TaskFilter::Completed => "completed_at IS NOT NULL",
        TaskFilter::All => "1=1",
    };
    let sql = format!(
        "SELECT id,title,due_at,completed_at,notified_at FROM tasks
         WHERE {condition} AND (?3 = '' OR instr(lower(title), lower(?3)) > 0)
         ORDER BY completed_at IS NOT NULL, due_at IS NULL, due_at ASC, created_at DESC LIMIT 200"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let result = stmt
        .query_map(params![now, tomorrow, search.trim()], decode)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string());
    result
}

pub fn list(filter: TaskFilter, search: &str) -> Result<Vec<Task>, String> {
    let today = Local::now().date_naive();
    let tomorrow = today.succ_opt().unwrap_or(today);
    // Day boundary in the user's local timezone, DST-aware.
    let midnight = tomorrow
        .and_hms_opt(0, 0, 0)
        .ok_or("Invalid day boundary")?;
    let end = Local
        .from_local_datetime(&midnight)
        .earliest()
        .ok_or("Cannot resolve local day boundary")?
        .timestamp_millis();
    list_with(&open()?, filter, search, now_ms(), end)
}

pub fn set_completed(id: i64, complete: bool) -> Result<(), String> {
    let conn = open()?;
    let now = now_ms();
    let count=conn.execute(
        "UPDATE tasks SET completed_at=?2,notified_at=CASE WHEN ?2 IS NULL THEN NULL ELSE notified_at END,
         updated_at=?3 WHERE id=?1",
        params![id, if complete {Some(now)} else {None},now],
    ).map_err(|e| e.to_string())?;
    if count == 0 {
        Err("Task no longer exists".into())
    } else {
        Ok(())
    }
}

pub fn reschedule(id: i64, due_at: Option<i64>) -> Result<(), String> {
    if due_at.is_some_and(|d| d <= now_ms()) {
        return Err("Choose a future reminder time".into());
    }
    let conn = open()?;
    let count=conn.execute(
        "UPDATE tasks SET due_at=?2,notified_at=NULL,updated_at=?3 WHERE id=?1 AND completed_at IS NULL",
        params![id,due_at,now_ms()],
    ).map_err(|e| e.to_string())?;
    if count == 0 {
        Err("Cannot reschedule a completed or missing task".into())
    } else {
        Ok(())
    }
}

fn due_with(conn: &Connection, now: i64) -> Result<Vec<Task>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id,title,due_at,completed_at,notified_at FROM tasks
         WHERE due_at IS NOT NULL AND due_at <= ?1 AND notified_at IS NULL
           AND completed_at IS NULL ORDER BY due_at LIMIT 10",
        )
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([now], decode)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string());
    result
}

pub fn due() -> Result<Vec<Task>, String> {
    due_with(&open()?, now_ms())
}

pub fn mark_notified(id: i64) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "UPDATE tasks SET notified_at=?2 WHERE id=?1 AND completed_at IS NULL
         AND notified_at IS NULL",
        params![id, now_ms()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    fn temp_db() -> (std::path::PathBuf, Connection) {
        let dir = std::env::temp_dir().join(format!(
            "my-tools-tasks-{}-{}-{}",
            std::process::id(),
            now_ms(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let conn = Connection::open(dir.join("tasks.db")).unwrap();
        init(&conn).unwrap();
        (dir, conn)
    }
    #[test]
    fn tasks_are_stored_filtered_and_searched() {
        let (dir, conn) = temp_db();
        let now = 1_000_000_i64;
        let a = add_with(&conn, "  Send project update  ", Some(now + 60_000), now).unwrap();
        add_with(&conn, "Buy milk", None, now).unwrap();
        assert_eq!(
            list_with(&conn, TaskFilter::All, "project", now, now + 86_400_000).unwrap()[0].id,
            a
        );
        assert_eq!(
            list_with(&conn, TaskFilter::Today, "", now, now + 86_400_000)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            list_with(&conn, TaskFilter::Upcoming, "", now, now + 86_400_000)
                .unwrap()
                .len(),
            1
        );
        assert!(due_with(&conn, now).unwrap().is_empty());
        assert_eq!(due_with(&conn, now + 60_000).unwrap().len(), 1);
        conn.execute(
            "UPDATE tasks SET completed_at=?2 WHERE id=?1",
            params![a, now + 90_000],
        )
        .unwrap();
        assert!(due_with(&conn, now + 100_000).unwrap().is_empty());
        assert_eq!(
            list_with(&conn, TaskFilter::Completed, "", now, now + 86_400_000)
                .unwrap()
                .len(),
            1
        );
        drop(conn);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn no_empty_task_or_past_reminder() {
        let (dir, conn) = temp_db();
        assert!(add_with(&conn, "    ", None, 100).is_err());
        assert!(add_with(&conn, "hello", Some(99), 100).is_err());
        assert!(parse_local_due("tomorrow").is_err());
        assert_eq!(parse_local_due(""), Ok(None));
        drop(conn);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn completed_tasks_do_not_reappear_as_reminders() {
        let (dir, conn) = temp_db();
        let id = add_with(&conn, "Reminder", Some(200), 100).unwrap();
        conn.execute("UPDATE tasks SET notified_at=201 WHERE id=?1", [id])
            .unwrap();
        assert!(due_with(&conn, 202).unwrap().is_empty());
        conn.execute("UPDATE tasks SET completed_at=203 WHERE id=?1", [id])
            .unwrap();
        assert!(due_with(&conn, 204).unwrap().is_empty());
        drop(conn);
        fs::remove_dir_all(dir).unwrap();
    }
}
