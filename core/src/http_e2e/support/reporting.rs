use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::super::HttpE2EReport;

pub(in crate::http_e2e) struct HttpE2EArchivePaths {
    pub(in crate::http_e2e) history_dir: PathBuf,
    pub(in crate::http_e2e) report_json: PathBuf,
    pub(in crate::http_e2e) report_markdown: PathBuf,
}

pub(super) fn render_markdown_report(report: &HttpE2EReport) -> String {
    let mut lines = vec![
        "# Allvia HTTP E2E".to_string(),
        "".to_string(),
        format!("- generated_at: {}", report.generated_at),
        format!("- status: {}", if report.ok { "ok" } else { "failed" }),
        format!("- steps: {}/{}", report.passed, report.total),
        format!("- api_base_url: {}", report.api_base_url),
        format!("- runtime_db_path: {}", report.runtime_db_path),
        format!("- digest_stub_url: {}", report.digest_stub_url),
        "".to_string(),
        "## Steps".to_string(),
        "".to_string(),
    ];

    for step in &report.steps {
        lines.push(format!(
            "- [{}] {}: {}",
            if step.ok { "pass" } else { "fail" },
            step.name,
            step.detail
        ));
    }

    lines.join("\n")
}

pub(super) fn archive_slug(generated_at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(generated_at)
        .map(|value| value.format("%Y%m%d-%H%M%S").to_string())
        .unwrap_or_else(|_| {
            generated_at
                .chars()
                .filter(|value| value.is_ascii_digit())
                .collect::<String>()
                .chars()
                .take(14)
                .collect::<String>()
        })
}

pub(in crate::http_e2e) fn build_archive_paths(
    workdir: &Path,
    generated_at: &str,
) -> HttpE2EArchivePaths {
    let slug = archive_slug(generated_at);
    let history_dir = workdir.join("reports/http_e2e/history").join(slug);
    HttpE2EArchivePaths {
        report_json: history_dir.join("http_e2e.json"),
        report_markdown: history_dir.join("http_e2e.md"),
        history_dir,
    }
}

pub(in crate::http_e2e) fn write_http_e2e_report(
    report: &HttpE2EReport,
    report_json_path: &Path,
    report_markdown_path: &Path,
    archive_paths: &HttpE2EArchivePaths,
) -> Result<()> {
    if let Some(parent) = report_json_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::create_dir_all(&archive_paths.history_dir)
        .with_context(|| format!("failed to create {}", archive_paths.history_dir.display()))?;

    let report_json =
        serde_json::to_string_pretty(report).context("failed to serialize http e2e report")?;
    let report_markdown = render_markdown_report(report);

    fs::write(report_json_path, &report_json)
        .with_context(|| format!("failed to write {}", report_json_path.display()))?;
    fs::write(report_markdown_path, &report_markdown)
        .with_context(|| format!("failed to write {}", report_markdown_path.display()))?;
    fs::write(&archive_paths.report_json, &report_json)
        .with_context(|| format!("failed to write {}", archive_paths.report_json.display()))?;
    fs::write(&archive_paths.report_markdown, &report_markdown).with_context(|| {
        format!(
            "failed to write {}",
            archive_paths.report_markdown.display()
        )
    })?;
    Ok(())
}
