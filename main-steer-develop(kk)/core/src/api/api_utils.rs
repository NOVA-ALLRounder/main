use std::io::Write;

pub fn list_files_human(path: &std::path::Path, limit: usize) -> String {
    let mut out = format!("Files in {}:\n", path.display());
    let read = std::fs::read_dir(path);
    match read {
        Ok(rd) => {
            let mut count = 0usize;
            for ent in rd.flatten() {
                if count >= limit {
                    out.push_str(&format!("... and more (showing first {})\n", limit));
                    break;
                }
                let p = ent.path();
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("(unknown)");
                if p.is_dir() {
                    out.push_str(&format!("[D] {}\n", name));
                } else {
                    out.push_str(&format!("[F] {}\n", name));
                }
                count += 1;
            }
            if count == 0 {
                out.push_str("(empty)\n");
            }
        }
        Err(e) => out.push_str(&format!("Failed to read dir: {}\n", e)),
    }
    out
}

pub fn open_url_windows(url: &str) -> Result<(), String> {
    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Start-Process \"{}\"", url.replace('"', "")),
        ])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Start-Process failed".to_string())
    }
}

pub fn open_path_windows(path: &std::path::Path) -> Result<(), String> {
    let p = path.to_string_lossy().replace('"', "");
    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Start-Process \"{}\"", p),
        ])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Start-Process failed".to_string())
    }
}

pub fn is_valid_notion_database_id(value: &str) -> bool {
    let trimmed = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('{')
        .trim_matches('}')
        .trim();
    let lowered = trimmed.to_lowercase();
    if trimmed.is_empty() || lowered.contains("your_database_id_here") {
        return false;
    }
    let normalized = trimmed.replace('-', "");
    normalized.len() == 32 && normalized.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn write_email_ops_artifacts(
    summary: &str,
    tasks: &[(String, String)],
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let base = std::env::var("STEER_HOME")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(".steer")
        })
        .join("artifacts")
        .join("email-ops")
        .join(chrono::Local::now().format("%Y%m%d-%H%M%S").to_string());
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;

    let summary_path = base.join("summary.md");
    let mut summary_file = std::fs::File::create(&summary_path).map_err(|e| e.to_string())?;
    summary_file
        .write_all(summary.as_bytes())
        .map_err(|e| e.to_string())?;

    let tasks_path = base.join("tasks.csv");
    let mut tasks_file = std::fs::File::create(&tasks_path).map_err(|e| e.to_string())?;
    tasks_file
        .write_all(b"priority,subject,from,action,owner,status\n")
        .map_err(|e| e.to_string())?;
    for (subject, from) in tasks {
        let line = format!(
            "P2,\"{}\",\"{}\",\"review-and-reply\",\"me\",\"todo\"\n",
            subject.replace('"', "'"),
            from.replace('"', "'")
        );
        tasks_file
            .write_all(line.as_bytes())
            .map_err(|e| e.to_string())?;
    }

    Ok((summary_path, tasks_path))
}
