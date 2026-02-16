pub(super) async fn get_system_health(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let health = crate::dependency_check::SystemHealth::check_all();
    let ctl = state
        .runtime_control
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());
    let mut value = serde_json::to_value(health).unwrap_or_else(|_| json!({}));
    if let Some(obj) = value.as_object_mut() {
        obj.insert("operation_mode".to_string(), json!(ctl.mode.as_str()));
        obj.insert("emergency_stop".to_string(), json!(ctl.emergency_stop));
        obj.insert("allow_automation".to_string(), json!(ctl.allow_automation()));
    }
    Json(value)
}

fn check_writable_dir(path: &std::path::Path) -> bool {
    if std::fs::create_dir_all(path).is_err() {
        return false;
    }
    let probe = path.join(".steer_write_probe");
    match std::fs::write(&probe, b"ok") {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

pub(super) async fn get_system_preflight(
    State(state): State<AppState>,
) -> Json<PreflightResponse> {
    let health = crate::dependency_check::SystemHealth::check_all();
    let ctl = state
        .runtime_control
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());

    let env_present = std::path::Path::new(".env").exists()
        || std::path::Path::new("core/.env").exists()
        || std::path::Path::new("web/src-tauri/target/release/.env").exists();

    let steer_home = std::env::var("STEER_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(".steer"));
    let steer_home_writable = check_writable_dir(&steer_home);

    let release_dir = std::env::current_dir()
        .unwrap_or_default()
        .join("web")
        .join("src-tauri")
        .join("target")
        .join("release");
    let release_dir_writable = check_writable_dir(&release_dir);

    let mut notes = Vec::new();
    if !health.api_reachable {
        notes.push("API not reachable on expected port".to_string());
    }
    if !env_present {
        notes.push(".env file not found in known runtime paths".to_string());
    }
    if !steer_home_writable {
        notes.push("STEER_HOME is not writable".to_string());
    }
    if !release_dir_writable {
        notes.push("Release runtime directory is not writable".to_string());
    }
    if !health.gmail_credentials_set {
        notes.push("Gmail credentials missing (core/credentials.json)".to_string());
    }
    if !health.notion_ready {
        notes.push("Notion integration not ready (NOTION_API_KEY/NOTION_DATABASE_ID)".to_string());
    }

    let ok = health.api_reachable
        && env_present
        && steer_home_writable
        && release_dir_writable;

    Json(PreflightResponse {
        ok,
        api_port: health.api_port,
        api_reachable: health.api_reachable,
        env_present,
        steer_home_writable,
        release_dir_writable,
        gmail_credentials_set: health.gmail_credentials_set,
        notion_ready: health.notion_ready,
        operation_mode: ctl.mode.as_str().to_string(),
        emergency_stop: ctl.emergency_stop,
        allow_automation: ctl.allow_automation(),
        notes,
    })
}

pub(super) async fn scan_project_handler(
    Query(query): Query<ProjectScanQuery>,
) -> Json<ProjectScanResponse> {
    let workdir = query
        .workdir
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().to_string());
    let scanner = project_scanner::ProjectScanner::new(&workdir);
    let result = scanner.scan(query.max_files);
    let project_type = scanner.get_project_type();

    Json(ProjectScanResponse {
        project_type: project_type.as_str().to_string(),
        files: result.files,
        key_files: result.key_files,
    })
}

pub(super) async fn run_runtime_verification_handler(
    Json(payload): Json<RuntimeVerifyRequest>,
) -> Json<runtime_verification::RuntimeVerifyResult> {
    let options = runtime_verification::RuntimeVerifyOptions {
        workdir: payload.workdir,
        run_backend: payload.run_backend,
        run_frontend: payload.run_frontend,
        run_e2e: payload.run_e2e,
        run_build_checks: payload.run_build_checks,
        backend_port: payload.backend_port,
        frontend_port: payload.frontend_port,
        backend_health_path: payload.backend_health_path,
    };
    let result = runtime_verification::run_runtime_verification(options).await;
    let summary = if result.issues.is_empty() {
        "Runtime verification passed".to_string()
    } else {
        format!("Runtime verification issues: {}", result.issues.len())
    };
    log_verification_run(
        "runtime",
        result.issues.is_empty(),
        &summary,
        Some(json!({ "issues": result.issues, "backend_health": result.backend_health, "frontend_health": result.frontend_health })),
    );
    Json(result)
}

pub(super) async fn run_visual_verification_handler(
    State(state): State<AppState>,
    Json(payload): Json<visual_verification::VisualVerifyRequest>,
) -> Json<visual_verification::VisualVerifyResult> {
    let Some(llm) = &state.llm_client else {
        return Json(visual_verification::VisualVerifyResult { ok: false, verdicts: vec![] });
    };
    match visual_verification::verify_screen(llm.as_ref(), payload).await {
        Ok(result) => {
            let summary = if result.ok { "Visual verification passed" } else { "Visual verification failed" };
            let details = json!({
                "verdicts": result.verdicts.iter().map(|v| json!({ "prompt": v.prompt, "ok": v.ok })).collect::<Vec<_>>()
            });
            log_verification_run("visual", result.ok, summary, Some(details));
            Json(result)
        }
        Err(_) => Json(visual_verification::VisualVerifyResult { ok: false, verdicts: vec![] }),
    }
}

pub(super) async fn run_semantic_verification_handler(
    Json(payload): Json<SemanticVerifyRequest>,
) -> Json<semantic_verification::SemanticVerificationResult> {
    let workdir = payload
        .workdir
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().to_string());
    let max_files = payload.max_files.unwrap_or(200);
    let result = semantic_verification::semantic_consistency(std::path::Path::new(&workdir), max_files);
    let details = json!({
        "issues": result.issues.iter().take(10).map(|i| json!({"file": i.file, "severity": i.severity, "reason": i.reason})).collect::<Vec<_>>()
    });
    log_verification_run("semantic", result.ok, &result.reason, Some(details));
    Json(result)
}

pub(super) async fn run_performance_verification_handler(
    Json(payload): Json<PerformanceVerifyRequest>,
) -> Json<performance_verification::PerformanceVerificationResult> {
    let workdir = payload
        .workdir
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().to_string());
    let max_files = payload.max_files.unwrap_or(300);
    let result = performance_verification::performance_baseline(std::path::Path::new(&workdir), max_files);
    let details = json!({
        "metrics": result.metrics.iter().map(|m| json!({"name": m.name, "value": m.value, "threshold": m.threshold, "ok": m.ok})).collect::<Vec<_>>()
    });
    log_verification_run("performance", result.ok, &result.reason, Some(details));
    Json(result)
}

pub(super) async fn run_consistency_verification_handler(
    Json(payload): Json<consistency_check::ConsistencyCheckRequest>,
) -> Json<consistency_check::ConsistencyCheckResult> {
    let result = consistency_check::run_consistency_check(payload);
    let details = json!({
        "summary": result.summary,
        "issues": result.issues.iter().take(10).map(|i| json!({"path": i.path, "source": i.source})).collect::<Vec<_>>()
    });
    log_verification_run("consistency", result.ok, &result.summary, Some(details));
    Json(result)
}

pub(super) async fn run_judgment_handler(
    Json(payload): Json<judgment::JudgmentRequest>,
) -> Json<judgment::JudgmentResponse> {
    let result = judgment::evaluate_judgment(payload);
    Json(result)
}

pub(super) async fn set_release_baseline_handler(
    Json(payload): Json<release_gate::ReleaseBaselineRequest>,
) -> Json<release_gate::ReleaseBaseline> {
    let baseline = release_gate::build_baseline(payload);
    release_gate::save_baseline(&baseline);
    Json(baseline)
}

pub(super) async fn run_release_gate_handler(
    Json(payload): Json<release_gate::ReleaseGateRequest>,
) -> Json<release_gate::ReleaseGateResult> {
    let result = release_gate::run_release_gate(payload);
    let summary = if result.ok { "Release gate passed" } else { "Release gate failed" };
    let details = json!({
        "regressions": result.regressions.iter().take(10).cloned().collect::<Vec<_>>(),
        "warnings": result.warnings.iter().take(10).cloned().collect::<Vec<_>>()
    });
    log_verification_run("release_gate", result.ok, summary, Some(details));
    Json(result)
}

pub(super) async fn run_exec_results_guard_handler(
    Json(payload): Json<tool_result_guard::ToolResultGuardRequest>,
) -> Json<tool_result_guard::ToolResultGuardResult> {
    let result = tool_result_guard::guard_exec_results(payload);
    Json(result)
}

pub(super) async fn score_quality_handler(
    State(state): State<AppState>,
    Json(payload): Json<QualityScoreRequest>,
) -> Json<QualityScoreResponse> {
    let runtime = if let Some(rt) = payload.runtime {
        rt
    } else if let Some(opts) = payload.runtime_options {
        let options = runtime_verification::RuntimeVerifyOptions {
            workdir: opts.workdir,
            run_backend: opts.run_backend,
            run_frontend: opts.run_frontend,
            run_e2e: opts.run_e2e,
            run_build_checks: opts.run_build_checks,
            backend_port: opts.backend_port,
            frontend_port: opts.frontend_port,
            backend_health_path: opts.backend_health_path,
        };
        runtime_verification::run_runtime_verification(options).await
    } else {
        runtime_verification::RuntimeVerifyResult {
            backend_started: false,
            backend_health: false,
            backend_build_ok: None,
            frontend_started: false,
            frontend_health: false,
            frontend_build_ok: None,
            e2e_passed: None,
            issues: vec!["No runtime verification provided".to_string()],
            logs: Vec::new(),
        }
    };

    let use_llm = payload.use_llm.unwrap_or(false);
    let score = if use_llm {
        if let Some(llm) = &state.llm_client {
            match quality_scorer::score_quality_with_llm(
                llm.as_ref(),
                payload.goal.as_deref(),
                Some(&runtime),
                payload.code_review.as_ref(),
            )
            .await
            {
                Ok(score) => score,
                Err(_) => quality_scorer::score_quality(Some(&runtime), payload.code_review.as_ref()),
            }
        } else {
            quality_scorer::score_quality(Some(&runtime), payload.code_review.as_ref())
        }
    } else {
        quality_scorer::score_quality(Some(&runtime), payload.code_review.as_ref())
    };
    let _ = db::insert_quality_score(&score);
    let created_at = chrono::Utc::now().to_rfc3339();
    Json(QualityScoreResponse { created_at, score })
}

pub(super) async fn latest_quality_handler() -> Json<Option<QualityScoreResponse>> {
    match db::get_latest_quality_score() {
        Ok(Some(record)) => {
            let score = quality_scorer::QualityScore {
                overall: record.overall,
                breakdown: record
                    .breakdown
                    .as_object()
                    .map(|map| {
                        map.iter()
                            .filter_map(|(k, v)| v.as_f64().map(|val| (k.clone(), val)))
                            .collect()
                    })
                    .unwrap_or_default(),
                issues: record.issues,
                strengths: record.strengths,
                recommendation: record.recommendation,
                summary: record.summary,
            };
            Json(Some(QualityScoreResponse {
                created_at: record.created_at,
                score,
            }))
        }
        _ => Json(None),
    }
}




