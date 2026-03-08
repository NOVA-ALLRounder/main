use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliArgs {
    config_path: PathBuf,
    refresh_candidates: bool,
    candidate_limit: usize,
    snapshot_output: Option<String>,
    candidate_mode: String,
}

fn parse_args<I>(args: I) -> anyhow::Result<CliArgs>
where
    I: IntoIterator<Item = String>,
{
    let mut config_path = None;
    let mut refresh_candidates = false;
    let mut candidate_limit = 12usize;
    let mut snapshot_output = None;
    let mut candidate_mode = "real".to_string();

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--refresh-candidates" => refresh_candidates = true,
            "--candidate-limit" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --candidate-limit"))?;
                candidate_limit = raw
                    .parse::<usize>()
                    .map_err(|_| anyhow::anyhow!("invalid --candidate-limit: {}", raw))?;
            }
            "--snapshot-output" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --snapshot-output"))?;
                snapshot_output = Some(raw);
            }
            "--candidate-mode" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --candidate-mode"))?;
                candidate_mode = raw;
            }
            value if value.starts_with("--") => {
                return Err(anyhow::anyhow!("unknown flag: {}", value));
            }
            value => {
                if config_path.is_some() {
                    return Err(anyhow::anyhow!("multiple config paths provided: {}", value));
                }
                config_path = Some(PathBuf::from(value));
            }
        }
    }

    Ok(CliArgs {
        config_path: config_path.unwrap_or_else(|| PathBuf::from("configs/launch_eval.yaml")),
        refresh_candidates,
        candidate_limit: candidate_limit.clamp(1, 50),
        snapshot_output,
        candidate_mode,
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    local_os_agent::load_env_with_fallback();

    let args = parse_args(std::env::args().skip(1))?;
    if args.refresh_candidates {
        let workdir = std::env::current_dir()?;
        let provenance_filter =
            local_os_agent::launch_eval::parse_launch_eval_candidate_provenance_filter(Some(
                &args.candidate_mode,
            ));
        let snapshot =
            local_os_agent::launch_eval::write_launch_eval_candidate_snapshot_with_filter(
                &workdir,
                args.snapshot_output.as_deref(),
                args.candidate_limit,
                provenance_filter,
            )?;
        println!(
            "Launch eval candidates refreshed: {} {} scenarios -> {}",
            snapshot.scenario_count, snapshot.provenance_filter, snapshot.output_path
        );
    }

    let report = local_os_agent::launch_eval::run_launch_eval_from_path(&args.config_path).await?;

    println!(
        "Allvia launch eval: {}/{} passed",
        report.passed, report.total
    );
    println!("JSON report: {}", report.report_json_path);
    println!("Markdown report: {}", report.report_markdown_path);

    if report.failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_args;
    use std::path::PathBuf;

    #[test]
    fn parse_args_defaults_to_launch_eval_config() {
        let parsed = parse_args(Vec::<String>::new()).expect("parse defaults");
        assert_eq!(
            parsed.config_path,
            PathBuf::from("configs/launch_eval.yaml")
        );
        assert!(!parsed.refresh_candidates);
        assert_eq!(parsed.candidate_limit, 12);
        assert_eq!(parsed.snapshot_output, None);
        assert_eq!(parsed.candidate_mode, "real");
    }

    #[test]
    fn parse_args_accepts_refresh_flags_and_config_path() {
        let parsed = parse_args(vec![
            "--refresh-candidates".to_string(),
            "--candidate-limit".to_string(),
            "25".to_string(),
            "--snapshot-output".to_string(),
            "configs/custom.generated.yaml".to_string(),
            "--candidate-mode".to_string(),
            "synthetic".to_string(),
            "configs/launch_eval.yaml".to_string(),
        ])
        .expect("parse refresh flags");
        assert!(parsed.refresh_candidates);
        assert_eq!(parsed.candidate_limit, 25);
        assert_eq!(
            parsed.snapshot_output.as_deref(),
            Some("configs/custom.generated.yaml")
        );
        assert_eq!(
            parsed.config_path,
            PathBuf::from("configs/launch_eval.yaml")
        );
        assert_eq!(parsed.candidate_mode, "synthetic");
    }

    #[test]
    fn parse_args_rejects_unknown_flag() {
        let err = parse_args(vec!["--unknown".to_string()]).expect_err("unknown flag should fail");
        assert!(err.to_string().contains("unknown flag"));
    }
}
