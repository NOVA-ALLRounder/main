use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::super::{HttpE2EHistoryEntry, HttpE2EReport};

pub fn latest_http_e2e_report_path(workdir: &Path) -> PathBuf {
    workdir.join("reports/http_e2e/latest.json")
}

pub fn load_latest_http_e2e_report(workdir: &Path) -> Result<Option<HttpE2EReport>> {
    let path = latest_http_e2e_report_path(workdir);
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let report = serde_json::from_str::<HttpE2EReport>(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(report))
}

pub fn list_http_e2e_history(workdir: &Path, limit: usize) -> Result<Vec<HttpE2EHistoryEntry>> {
    let limit = limit.clamp(1, 100);
    let history_dir = workdir.join("reports/http_e2e/history");
    if !history_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(&history_dir)
        .with_context(|| format!("failed to read {}", history_dir.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path().join("http_e2e.json");
            if !path.exists() {
                return None;
            }
            let payload = fs::read_to_string(&path).ok()?;
            let report = serde_json::from_str::<HttpE2EReport>(&payload).ok()?;
            Some(HttpE2EHistoryEntry {
                generated_at: report.generated_at,
                ok: report.ok,
                passed: report.passed,
                total: report.total,
                report_json_path: path.display().to_string(),
                report_markdown_path: path.with_file_name("http_e2e.md").display().to_string(),
            })
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| right.generated_at.cmp(&left.generated_at));
    entries.truncate(limit);
    Ok(entries)
}
