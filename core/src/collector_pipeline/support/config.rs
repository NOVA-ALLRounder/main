use super::super::*;

pub(crate) fn config_db_path(config_path: Option<&Path>) -> Option<PathBuf> {
    let cfg_path = config_path?;
    let cfg = read_config(cfg_path)?;
    let db_raw = cfg.db_path?.trim().to_string();
    if db_raw.is_empty() {
        return None;
    }

    if is_default_collector_db_path(&db_raw) {
        if let Some(path) =
            env_path("STEER_COLLECTOR_DB_PATH").or_else(|| env_path("STEER_DB_PATH"))
        {
            return Some(path);
        }
    }

    let candidate = PathBuf::from(&db_raw);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        cfg_path
            .parent()
            .map(|p| p.join(&candidate))
            .or(Some(candidate))
            .unwrap_or_else(|| PathBuf::from(&db_raw))
    };

    if is_default_collector_db_path(&db_raw) {
        if collector_separate_db_enabled() {
            return Some(resolved);
        }
        if collector_auto_link_enabled() {
            if let Some(shared) = discover_shared_steer_db() {
                return Some(shared);
            }
        }
    }
    Some(resolved)
}

pub(crate) fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub(crate) fn env_bool_with_default(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

pub(crate) fn collector_auto_link_enabled() -> bool {
    env_bool_with_default("STEER_COLLECTOR_AUTO_LINK", true)
}

pub(crate) fn collector_separate_db_enabled() -> bool {
    env_bool_with_default("STEER_COLLECTOR_SEPARATE_DB", false)
}

pub(crate) fn is_default_collector_db_path(raw: &str) -> bool {
    let normalized = raw.trim().replace('\\', "/").to_ascii_lowercase();
    normalized == "collector.db" || normalized == "./collector.db"
}

pub(crate) fn discover_shared_steer_db() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(mut base) = dirs::data_local_dir() {
        base.push("steer");
        base.push("steer.db");
        candidates.push(base);
    }

    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            let home_path = PathBuf::from(home);
            candidates.push(
                home_path
                    .join("Library")
                    .join("Application Support")
                    .join("steer")
                    .join("steer.db"),
            );
            candidates.push(
                home_path
                    .join(".local")
                    .join("share")
                    .join("steer")
                    .join("steer.db"),
            );
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("steer.db"));
        candidates.push(cwd.join("core").join("steer.db"));
        candidates.push(cwd.join("..").join("steer.db"));
        candidates.push(cwd.join("..").join("core").join("steer.db"));
    }

    candidates.into_iter().find(|path| path.is_file())
}

pub(crate) fn ensure_column_exists(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    if table_has_column(conn, table, column)? {
        return Ok(());
    }
    let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
    conn.execute(&sql, [])?;
    Ok(())
}

pub(crate) fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&pragma)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name.eq_ignore_ascii_case(column) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn config_privacy_path(config_path: Option<&Path>) -> Option<PathBuf> {
    let cfg_path = config_path?;
    let cfg = read_config(cfg_path)?;
    let raw = cfg.privacy_rules_path?.trim().to_string();
    if raw.is_empty() {
        return None;
    }
    let candidate = PathBuf::from(&raw);
    if candidate.is_absolute() {
        Some(candidate)
    } else {
        cfg_path
            .parent()
            .map(|p| p.join(&candidate))
            .or(Some(candidate))
    }
}

pub(crate) fn read_config(path: &Path) -> Option<ConfigYaml> {
    let text = fs::read_to_string(path).ok()?;
    serde_yaml::from_str::<ConfigYaml>(&text).ok()
}
