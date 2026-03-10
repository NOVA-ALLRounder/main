use lazy_static::lazy_static;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;
use std::sync::Mutex;

use super::{init_sessions_table, init_v2, seed_advanced_examples};

#[path = "bootstrap/schema.rs"]
mod schema;
use schema::{apply_base_schema, run_post_init_repairs};

lazy_static! {
    static ref DB_CONN: Mutex<Option<Connection>> = Mutex::new(None);
    static ref DB_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
    static ref DB_INIT_LOCK: Mutex<()> = Mutex::new(());
}

pub(crate) fn get_db_lock() -> std::sync::MutexGuard<'static, Option<Connection>> {
    match DB_CONN.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("⚠️ DB Mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    }
}

fn get_db_path_lock() -> std::sync::MutexGuard<'static, Option<PathBuf>> {
    match DB_PATH.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("⚠️ DB path mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    }
}

pub fn current_db_path() -> Option<String> {
    if let Some(path) = get_db_path_lock().as_ref() {
        return Some(path.display().to_string());
    }
    let mut lock = get_db_lock();
    let conn = lock.as_mut()?;
    let mut stmt = conn.prepare("PRAGMA database_list").ok()?;
    let mut rows = stmt.query([]).ok()?;
    while let Ok(Some(row)) = rows.next() {
        let name: String = row.get(1).ok()?;
        if name == "main" {
            let path: String = row.get(2).ok()?;
            if !path.trim().is_empty() {
                return Some(path);
            }
        }
    }
    None
}

pub fn reset_connection() {
    let mut lock = get_db_lock();
    if let Some(conn) = lock.take() {
        drop(conn);
    }
    *get_db_path_lock() = None;
}

pub(crate) fn open_readonly_connection() -> rusqlite::Result<Connection> {
    let stored_path = { get_db_path_lock().as_ref().cloned() };
    let path = stored_path
        .or_else(|| current_db_path().map(PathBuf::from))
        .ok_or(rusqlite::Error::InvalidQuery)?;

    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(2))?;
    conn.execute_batch("PRAGMA query_only = ON; PRAGMA busy_timeout = 2000;")?;
    Ok(conn)
}

pub(crate) fn open_write_connection() -> rusqlite::Result<Connection> {
    let stored_path = { get_db_path_lock().as_ref().cloned() };
    let path = stored_path
        .or_else(|| current_db_path().map(PathBuf::from))
        .ok_or(rusqlite::Error::InvalidQuery)?;

    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(conn)
}

pub(crate) fn with_read_conn<T, F>(mut op: F) -> rusqlite::Result<T>
where
    F: FnMut(&Connection) -> rusqlite::Result<T>,
{
    if let Ok(conn) = open_readonly_connection() {
        return op(&conn);
    }

    let mut lock = get_db_lock();
    let conn = lock.as_mut().ok_or(rusqlite::Error::InvalidQuery)?;
    op(conn)
}

pub(crate) fn with_write_conn_if_available<T, F>(mut op: F) -> rusqlite::Result<Option<T>>
where
    F: FnMut(&mut Connection) -> rusqlite::Result<T>,
{
    if let Ok(mut conn) = open_write_connection() {
        return op(&mut conn).map(Some);
    }

    let mut lock = get_db_lock();
    let Some(conn) = lock.as_mut() else {
        return Ok(None);
    };
    op(conn).map(Some)
}

pub(crate) fn ensure_approval_decisions_table(conn: &Connection) {
    if let Err(e) = conn.execute(
        "CREATE TABLE IF NOT EXISTS nl_approval_decisions (
            decision_key TEXT PRIMARY KEY,
            plan_id TEXT NOT NULL,
            action TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL
        )",
        [],
    ) {
        eprintln!("Failed to create nl_approval_decisions table: {}", e);
        return;
    }
    if let Err(e) = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_nl_approval_decisions_expires_at
         ON nl_approval_decisions(expires_at)",
        [],
    ) {
        eprintln!(
            "Failed to create idx_nl_approval_decisions_expires_at index: {}",
            e
        );
    }
}

pub fn init() -> anyhow::Result<()> {
    let _init_guard = match DB_INIT_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("⚠️ DB init mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };

    {
        let lock = get_db_lock();
        if lock.is_some() {
            return Ok(());
        }
    }

    let db_path = if let Ok(override_path) = std::env::var("STEER_DB_PATH") {
        let trimmed = override_path.trim();
        if trimmed.is_empty() {
            std::path::PathBuf::from("steer.db")
        } else {
            std::path::PathBuf::from(trimmed)
        }
    } else {
        #[cfg(test)]
        {
            std::env::temp_dir().join("steer_test.db")
        }
        #[cfg(not(test))]
        {
            if let Some(mut path) = dirs::data_local_dir() {
                path.push("steer");
                std::fs::create_dir_all(&path)?;
                path.push("steer.db");
                path
            } else {
                std::path::PathBuf::from("steer.db")
            }
        }
    };

    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let conn = Connection::open(&db_path)?;
    println!("📦 Database initialized at: {:?}", db_path);
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;

    apply_base_schema(&conn)?;

    {
        let mut lock = get_db_lock();
        *lock = Some(conn);
    }
    *get_db_path_lock() = Some(db_path);

    println!("📦 Database 'steer.db' initialized.");

    if let Err(e) = init_v2() {
        eprintln!("Failed to init events_v2: {}", e);
    }
    if let Err(e) = init_sessions_table() {
        eprintln!("Failed to init sessions_v2: {}", e);
    }

    if let Err(e) = seed_advanced_examples() {
        eprintln!("Failed to seed templates: {}", e);
    }

    if let Some(conn) = get_db_lock().as_mut() {
        run_post_init_repairs(conn);
    }

    Ok(())
}
