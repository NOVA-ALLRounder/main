use axum::{
    extract::Query,
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Deserialize)]
pub struct DcpWorkflowRunRequest {
    pub config_path: Option<String>,
    pub output_dir: Option<String>,
    pub use_quality_loop: Option<bool>,
    pub send_to_n8n: Option<bool>,
    pub webhook: Option<String>,
    pub min_score: Option<u32>,
    pub max_attempts: Option<u32>,
}

#[derive(Serialize)]
pub struct DcpWorkflowRunResponse {
    pub ok: bool,
    pub message: String,
    pub dcp_root: String,
    pub output_dir: String,
    pub workflow_path: Option<String>,
    pub quality_loop: bool,
    pub sent_to_n8n: bool,
}

#[derive(Deserialize)]
pub struct DcpWorkflowStatusQuery {
    pub output_dir: Option<String>,
}

#[derive(Serialize)]
pub struct DcpWorkflowStatusResponse {
    pub ok: bool,
    pub dcp_root: String,
    pub output_dir: String,
    pub workflow_path: Option<String>,
    pub workflow_updated_at: Option<String>,
    pub llm_input_path: Option<String>,
    pub recommendations_json_path: Option<String>,
    pub delivery_log_path: Option<String>,
}

fn resolve_dcp_root() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("STEER_DCP_ROOT") {
        let p = PathBuf::from(raw);
        if p.exists() {
            return Some(p);
        }
    }

    let cwd = std::env::current_dir().ok()?;
    let candidates = vec![
        cwd.join("collector").join("Data-Collection-Projection"),
        cwd.join("..").join("collector").join("Data-Collection-Projection"),
        cwd.join("..").join("..").join("collector").join("Data-Collection-Projection"),
        cwd.join("..")
            .join("..")
            .join("..")
            .join("collector")
            .join("Data-Collection-Projection"),
    ];

    candidates.into_iter().find(|p| p.exists())
}

fn file_path_if_exists(path: PathBuf) -> Option<String> {
    if path.exists() {
        Some(path.display().to_string())
    } else {
        None
    }
}

fn file_modified_at(path: &std::path::Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let dt: chrono::DateTime<chrono::Utc> = modified.into();
    Some(dt.to_rfc3339())
}

pub async fn get_dcp_workflow_status_handler(
    Query(query): Query<DcpWorkflowStatusQuery>,
) -> Json<DcpWorkflowStatusResponse> {
    let Some(dcp_root) = resolve_dcp_root() else {
        return Json(DcpWorkflowStatusResponse {
            ok: false,
            dcp_root: "".to_string(),
            output_dir: "".to_string(),
            workflow_path: None,
            workflow_updated_at: None,
            llm_input_path: None,
            recommendations_json_path: None,
            delivery_log_path: None,
        });
    };

    let output_dir = query.output_dir.unwrap_or_else(|| "logs".to_string());
    let out = dcp_root.join(&output_dir);
    let wf = out.join("n8n_workflow.json");
    let llm_input = out.join("llm_input.json");
    let rec_json = out.join("activity_recommendations.json");
    let delivery_log = out.join("n8n_delivery.log");

    Json(DcpWorkflowStatusResponse {
        ok: true,
        dcp_root: dcp_root.display().to_string(),
        output_dir: out.display().to_string(),
        workflow_path: file_path_if_exists(wf.clone()),
        workflow_updated_at: file_modified_at(&wf),
        llm_input_path: file_path_if_exists(llm_input),
        recommendations_json_path: file_path_if_exists(rec_json),
        delivery_log_path: file_path_if_exists(delivery_log),
    })
}

pub async fn run_dcp_workflow_handler(
    Json(payload): Json<DcpWorkflowRunRequest>,
) -> Json<DcpWorkflowRunResponse> {
    let Some(dcp_root) = resolve_dcp_root() else {
        return Json(DcpWorkflowRunResponse {
            ok: false,
            message:
                "DCP root not found. Set STEER_DCP_ROOT or verify collector/Data-Collection-Projection"
                    .to_string(),
            dcp_root: "".to_string(),
            output_dir: "".to_string(),
            workflow_path: None,
            quality_loop: false,
            sent_to_n8n: false,
        });
    };

    let scripts_dir = dcp_root.join("scripts");
    let cfg = payload
        .config_path
        .clone()
        .unwrap_or_else(|| "configs/config.yaml".to_string());
    let out_rel = payload.output_dir.clone().unwrap_or_else(|| "logs".to_string());
    let out_dir = dcp_root.join(&out_rel);
    let llm_input = out_dir.join("llm_input.json");
    let workflow = out_dir.join("n8n_workflow.json");
    let profile = dcp_root.join("configs/personalization_demo.json");
    let score_cfg = dcp_root.join("configs/score_config.json");
    let use_quality_loop = payload.use_quality_loop.unwrap_or(false);
    let min_score = payload.min_score.unwrap_or(85);
    let max_attempts = payload.max_attempts.unwrap_or(3);

    if !out_dir.exists() {
        let _ = std::fs::create_dir_all(&out_dir);
    }

    if !llm_input.exists() {
        return Json(DcpWorkflowRunResponse {
            ok: false,
            message: format!("llm_input.json missing: {}", llm_input.display()),
            dcp_root: dcp_root.display().to_string(),
            output_dir: out_dir.display().to_string(),
            workflow_path: None,
            quality_loop: use_quality_loop,
            sent_to_n8n: false,
        });
    }

    let mut cmd = if use_quality_loop {
        let mut c = Command::new("python");
        c.current_dir(&dcp_root)
            .arg(scripts_dir.join("generate_workflow_with_retry.py"))
            .arg("--config")
            .arg(cfg)
            .arg("--input")
            .arg(llm_input.display().to_string())
            .arg("--output")
            .arg(workflow.display().to_string())
            .arg("--profile")
            .arg(profile.display().to_string())
            .arg("--score-config")
            .arg(score_cfg.display().to_string())
            .arg("--min-score")
            .arg(min_score.to_string())
            .arg("--max-attempts")
            .arg(max_attempts.to_string());
        c
    } else {
        let mut c = Command::new("python");
        c.current_dir(&dcp_root)
            .arg(scripts_dir.join("generate_n8n_workflow.py"))
            .arg("--config")
            .arg(cfg)
            .arg("--input")
            .arg(llm_input.display().to_string())
            .arg("--output")
            .arg(workflow.display().to_string())
            .arg("--profile")
            .arg(profile.display().to_string());
        c
    };

    let generate_out = match cmd.output() {
        Ok(v) => v,
        Err(e) => {
            return Json(DcpWorkflowRunResponse {
                ok: false,
                message: format!("Failed to run workflow generator: {}", e),
                dcp_root: dcp_root.display().to_string(),
                output_dir: out_dir.display().to_string(),
                workflow_path: None,
                quality_loop: use_quality_loop,
                sent_to_n8n: false,
            });
        }
    };

    if !generate_out.status.success() || !workflow.exists() {
        let stderr = String::from_utf8_lossy(&generate_out.stderr);
        let stdout = String::from_utf8_lossy(&generate_out.stdout);
        return Json(DcpWorkflowRunResponse {
            ok: false,
            message: format!(
                "Workflow generation failed. stdout={} stderr={}",
                stdout.chars().take(240).collect::<String>(),
                stderr.chars().take(240).collect::<String>()
            ),
            dcp_root: dcp_root.display().to_string(),
            output_dir: out_dir.display().to_string(),
            workflow_path: file_path_if_exists(workflow),
            quality_loop: use_quality_loop,
            sent_to_n8n: false,
        });
    }

    let mut sent = false;
    if payload.send_to_n8n.unwrap_or(false) {
        let webhook = payload
            .webhook
            .clone()
            .or_else(|| std::env::var("N8N_WEBHOOK_URL").ok())
            .unwrap_or_default();
        if !webhook.trim().is_empty() {
            let send_out = Command::new("python")
                .current_dir(&dcp_root)
                .arg(scripts_dir.join("send_n8n_workflow.py"))
                .arg("--file")
                .arg(workflow.display().to_string())
                .arg("--webhook")
                .arg(webhook)
                .output();
            if let Ok(out) = send_out {
                sent = out.status.success();
            }
        }
    }

    Json(DcpWorkflowRunResponse {
        ok: true,
        message: "DCP workflow generation completed".to_string(),
        dcp_root: dcp_root.display().to_string(),
        output_dir: out_dir.display().to_string(),
        workflow_path: file_path_if_exists(workflow),
        quality_loop: use_quality_loop,
        sent_to_n8n: sent,
    })
}
