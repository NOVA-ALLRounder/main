use std::path::{Path, PathBuf};

pub fn home_dir() -> PathBuf {
    dirs::home_dir()
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn app_dir() -> PathBuf {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(custom) = std::env::var("STEER_HOME") {
        let custom = custom.trim();
        if !custom.is_empty() {
            candidates.push(PathBuf::from(custom));
        }
    }

    if let Some(mut local) = dirs::data_local_dir() {
        local.push("steer");
        candidates.push(local);
    }

    candidates.push(home_dir().join(".steer"));
    candidates.push(std::env::temp_dir().join("steer"));
    candidates.push(
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".steer"),
    );

    for candidate in &candidates {
        if ensure_writable_dir(candidate) {
            return candidate.clone();
        }
    }

    candidates
        .into_iter()
        .last()
        .unwrap_or_else(|| PathBuf::from(".steer"))
}

pub fn temp_file(prefix: &str, extension: &str) -> PathBuf {
    let clean_ext = extension.trim().trim_start_matches('.');
    if clean_ext.is_empty() {
        std::env::temp_dir().join(format!("{}_{}", prefix, uuid::Uuid::new_v4()))
    } else {
        std::env::temp_dir().join(format!("{}_{}.{}", prefix, uuid::Uuid::new_v4(), clean_ext))
    }
}

fn ensure_writable_dir(path: &Path) -> bool {
    if std::fs::create_dir_all(path).is_err() {
        return false;
    }

    let probe = path.join(format!(".write_probe_{}", std::process::id()));
    let write_ok = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&probe)
        .is_ok();
    let _ = std::fs::remove_file(&probe);
    write_ok
}
