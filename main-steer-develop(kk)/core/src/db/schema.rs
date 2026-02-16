use rusqlite::Connection;

pub(crate) fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    let sql = format!("PRAGMA table_info({})", table);
    let mut stmt = match conn.prepare(&sql) {
        Ok(stmt) => stmt,
        Err(e) => {
            eprintln!("Failed to read table_info for {}: {}", table, e);
            return false;
        }
    };
    let rows = match stmt.query_map([], |row| row.get::<_, String>(1)) {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("Failed to query table_info for {}: {}", table, e);
            return false;
        }
    };
    for name in rows {
        if let Ok(name) = name {
            if name == column {
                return true;
            }
        }
    }
    false
}

pub(crate) fn ensure_column(conn: &Connection, table: &str, column: &str, ddl: &str) {
    if column_exists(conn, table, column) {
        return;
    }
    let sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, ddl);
    if let Err(e) = conn.execute(&sql, []) {
        eprintln!("Failed to add column {}.{}: {}", table, column, e);
    }
}

