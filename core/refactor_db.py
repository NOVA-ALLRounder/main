#!/usr/bin/env python3
"""
Refactor db.rs to use r2d2 connection pooling instead of Mutex<Option<Connection>>.
"""

import re

def refactor_db_file(filepath):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    # 1. Update imports and global connection
    old_imports = """#![allow(dead_code)] // Allow unused library functions for future use
use rusqlite::{params, Connection, Result};

use std::sync::Mutex;
use lazy_static::lazy_static;
use crate::recommendation::AutomationProposal;
use crate::quality_scorer::QualityScore;
use crate::privacy::PrivacyGuard;
use std::str::FromStr; // Added

// Global DB connection (for MVP simplicity)
// In production, we should pass a connection pool or handle.
// But rusqlite Connection is not thread-safe, so we wrap in Mutex.
lazy_static! {
    static ref DB_CONN: Mutex<Option<Connection>> = Mutex::new(None);
}

/// Safe helper to acquire DB lock. Recovers from poisoned mutex.
fn get_db_lock() -> std::sync::MutexGuard<'static, Option<Connection>> {
    match DB_CONN.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("⚠️ DB Mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    }
}"""

    new_imports = """#![allow(dead_code)] // Allow unused library functions for future use
use rusqlite::{params, Connection, Result};

use lazy_static::lazy_static;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use crate::recommendation::AutomationProposal;
use crate::quality_scorer::QualityScore;
use crate::privacy::PrivacyGuard;
use std::str::FromStr; // Added

// Global DB connection pool for concurrent access
lazy_static! {
    static ref DB_POOL: std::sync::RwLock<Option<Pool<SqliteConnectionManager>>> = std::sync::RwLock::new(None);
}

/// Get a connection from the pool
fn get_connection() -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
    let pool_lock = DB_POOL.read().unwrap();
    match pool_lock.as_ref() {
        Some(pool) => pool.get().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!("Failed to get connection from pool: {}", e)),
            )
        }),
        None => Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some("DB pool not initialized".to_string()),
        )),
    }
}"""

    content = content.replace(old_imports, new_imports)

    # 2. Update init() function - idempotency check
    content = re.sub(
        r'    // \[Paranoid Audit\] Fix Connection Leak & Idempotency\n    \{\n        let lock = get_db_lock\(\);\n        if lock\.is_some\(\) \{',
        '    // [Paranoid Audit] Fix Connection Leak & Idempotency\n    {\n        let lock = DB_POOL.read().unwrap();\n        if lock.is_some() {',
        content
    )

    # 3. Update init() - create pool instead of single connection
    old_init_connection = """    // Open (or create) steer.db
    let conn = Connection::open(&db_path)?;
    println!("📦 Database initialized at: {:?}", db_path);

    // [Paranoid Audit] Set Busy Timeout to 5s to handle concurrency (Analyzer + API + Main)
    conn.busy_timeout(std::time::Duration::from_secs(5))?;"""

    new_init_connection = """    println!("📦 Initializing database connection pool at: {:?}", db_path);

    // Create connection pool manager with WAL mode and busy timeout
    let manager = SqliteConnectionManager::file(&db_path)
        .with_init(|conn| {
            // Enable WAL mode for concurrent read/write access
            conn.execute_batch("PRAGMA journal_mode=WAL;")?;
            // Set busy timeout to 5s to handle concurrency
            conn.busy_timeout(std::time::Duration::from_secs(5))?;
            Ok(())
        });

    // Build connection pool with max 10 connections
    let pool = Pool::builder()
        .max_size(10)
        .build(manager)?;

    // Get a connection from pool to initialize schema
    let conn = pool.get().map_err(|e| anyhow::anyhow!("Failed to get connection: {}", e))?;"""

    content = content.replace(old_init_connection, new_init_connection)

    # 4. Update init() - store pool instead of connection
    old_store = """
    // Store connection
    {
        let mut lock = get_db_lock();
        *lock = Some(conn);
    } // Lock is dropped here

    println!("📦 Database 'steer.db' initialized.");

    // Init V2 Schema
    {
        // Must release lock before calling init_v2 if it grabs lock?
        // Actually init_v2 grabs lock. But here we already dropped the lock scope in line 79.
    }"""

    new_store = """
    // Release connection back to pool (automatically happens when conn is dropped)
    drop(conn);

    // Store pool globally
    {
        let mut pool_lock = DB_POOL.write().unwrap();
        *pool_lock = Some(pool);
    }

    println!("📦 Database pool initialized with 10 connections (WAL mode enabled)");

    // Init V2 Schema
    // Connection pool allows concurrent access, no lock issues"""

    content = content.replace(old_store, new_store)

    # 5. Update the ensure_column section in init()
    old_ensure = """    // [Migration] Ensure 'evidence' column exists
    if let Some(conn) = get_db_lock().as_mut() {"""

    new_ensure = """    // [Migration] Ensure 'evidence' column exists
    if let Ok(conn) = get_connection() {"""

    content = content.replace(old_ensure, new_ensure)

    # Also update the ensure_column calls to use &conn instead of conn
    content = re.sub(
        r'ensure_column\(\n            conn,',
        'ensure_column(\n            &conn,',
        content
    )
    content = re.sub(
        r'ensure_column\(conn,',
        'ensure_column(&conn,',
        content
    )

    # 6. Refactor all function patterns
    # Pattern: let mut lock = get_db_lock();\n    if let Some(conn) = lock.as_mut() {
    # We need to be careful about indentation and matching braces

    # Simple replacement for the start
    content = re.sub(
        r'let mut lock = get_db_lock\(\);',
        'let conn = get_connection()?;',
        content
    )

    # Now remove the if let Some(conn) = lock.as_mut() { pattern
    # This is tricky because we need to match the closing brace
    # Let's do a simple pattern that works for most cases
    content = re.sub(
        r'\n    if let Some\(conn\) = lock\.as_mut\(\) \{\n',
        '\n',
        content
    )

    # Remove common else patterns
    content = re.sub(
        r'\n    \} else \{\n        Err\(rusqlite::Error::SqliteFailure\(\n            rusqlite::ffi::Error::new\(1\),\n            Some\("DB not initialized"\.to_string\(\)\),\n        \)\)\n    \}',
        '',
        content
    )

    content = re.sub(
        r'\n    \} else \{\n        Ok\(Vec::new\(\)\)\n    \}',
        '',
        content
    )

    content = re.sub(
        r'\n    \} else \{\n        Ok\(\(\)\)\n    \}',
        '',
        content
    )

    content = re.sub(
        r'\n    \} else \{\n        Ok\(false\)\n    \}',
        '',
        content
    )

    content = re.sub(
        r'\n    \} else \{\n        Ok\(0\)\n    \}',
        '',
        content
    )

    content = re.sub(
        r'\n    \} else \{\n        Ok\(None\)\n    \}',
        '',
        content
    )

    # Clean up excessive closing braces
    # After removing the if let Some pattern, we have an extra closing brace
    # This is harder to handle programmatically, so we'll need manual fixes

    return content

if __name__ == '__main__':
    filepath = r'c:\Users\Admin\Desktop\steer\core\src\db.rs'

    print("Refactoring db.rs...")
    refactored = refactor_db_file(filepath)

    # Write to a temporary file first
    temp_path = filepath + '.refactored'
    with open(temp_path, 'w', encoding='utf-8') as f:
        f.write(refactored)

    print(f"Refactored content written to {temp_path}")
    print("Please review the changes, then rename to db.rs if correct")
