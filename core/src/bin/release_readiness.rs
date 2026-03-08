use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliArgs {
    config_path: PathBuf,
    workdir: Option<PathBuf>,
    candidate_limit: usize,
    snapshot_output: Option<String>,
    save_baseline: bool,
}

fn parse_args<I>(args: I) -> anyhow::Result<CliArgs>
where
    I: IntoIterator<Item = String>,
{
    let mut config_path = None;
    let mut workdir = None;
    let mut candidate_limit = 20usize;
    let mut snapshot_output = None;
    let mut save_baseline = false;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--workdir" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --workdir"))?;
                workdir = Some(PathBuf::from(raw));
            }
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
            "--save-baseline" => save_baseline = true,
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
        workdir,
        candidate_limit: candidate_limit.clamp(1, 50),
        snapshot_output,
        save_baseline,
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    local_os_agent::load_env_with_fallback();

    let args = parse_args(std::env::args().skip(1))?;
    let workdir = args.workdir.unwrap_or(std::env::current_dir()?);
    let report = local_os_agent::release_readiness::run_release_readiness(
        &workdir,
        &args.config_path,
        args.candidate_limit,
        args.snapshot_output.as_deref(),
        args.save_baseline,
    )
    .await?;

    println!(
        "Allvia release readiness: status={} eval={}/{} snapshot={}",
        report.status,
        report.launch_eval.passed,
        report.launch_eval.total,
        report.candidate_snapshot.scenario_count
    );
    println!("JSON report: {}", report.report_json_path);
    println!("Markdown report: {}", report.report_markdown_path);

    if !report.ready_for_launch {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_args;
    use std::path::PathBuf;

    #[test]
    fn parse_args_defaults() {
        let parsed = parse_args(Vec::<String>::new()).expect("parse defaults");
        assert_eq!(
            parsed.config_path,
            PathBuf::from("configs/launch_eval.yaml")
        );
        assert_eq!(parsed.candidate_limit, 20);
        assert_eq!(parsed.workdir, None);
        assert_eq!(parsed.snapshot_output, None);
        assert!(!parsed.save_baseline);
    }

    #[test]
    fn parse_args_accepts_flags() {
        let parsed = parse_args(vec![
            "--workdir".to_string(),
            "/tmp/allvia".to_string(),
            "--candidate-limit".to_string(),
            "12".to_string(),
            "--snapshot-output".to_string(),
            "configs/custom.yaml".to_string(),
            "--save-baseline".to_string(),
            "configs/launch_eval.yaml".to_string(),
        ])
        .expect("parse flags");
        assert_eq!(parsed.workdir, Some(PathBuf::from("/tmp/allvia")));
        assert_eq!(parsed.candidate_limit, 12);
        assert_eq!(
            parsed.snapshot_output.as_deref(),
            Some("configs/custom.yaml")
        );
        assert!(parsed.save_baseline);
    }
}
