use lazy_static::lazy_static;
use rusqlite::Connection;
use std::sync::Mutex;

use super::{init_sessions_table, init_v2, seed_advanced_examples};

#[path = "bootstrap/schema.rs"]
mod schema;
use schema::{apply_base_schema, run_post_init_repairs};

lazy_static! {
    static ref DB_CONN: Mutex<Option<Connection>> = Mutex::new(None);
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

pub fn current_db_path() -> Option<String> {
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

    apply_base_schema(&conn)?;

    {
        let mut lock = get_db_lock();
        *lock = Some(conn);
    }

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
