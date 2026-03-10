use std::path::PathBuf;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let workdir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    match local_os_agent::http_e2e::run_http_e2e(&workdir).await {
        Ok(report) => {
            println!(
                "Allvia HTTP E2E: status={} steps={}/{}",
                if report.ok { "ok" } else { "failed" },
                report.passed,
                report.total
            );
            println!("JSON report: {}", report.report_json_path);
            println!("Markdown report: {}", report.report_markdown_path);
            if report.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("Allvia HTTP E2E failed: {:#}", error);
            ExitCode::from(1)
        }
    }
}
