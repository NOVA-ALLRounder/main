#[path = "schemas/action_schema.rs"]
pub mod action_schema;
#[path = "analysis_core/analyzer.rs"]
pub mod analyzer;
#[path = "ui_automation/applescript.rs"]
pub mod applescript;
#[path = "agent/approval_gate.rs"]
pub mod approval_gate;
#[path = "automation/bash_executor.rs"]
pub mod bash_executor;
#[path = "ui_automation/browser_automation.rs"]
pub mod browser_automation;
pub mod controller;
pub mod db;
#[path = "db/schema.rs"]
pub mod db_schema;
#[path = "ops/dependency_check.rs"]
pub mod dependency_check;
#[path = "integration_core/external_apis.rs"]
pub mod external_apis;
#[path = "integration_core/llm_gateway.rs"]
pub mod llm_gateway;
#[path = "integration_core/mcp_client.rs"]
pub mod mcp_client;
#[path = "ops/monitor.rs"]
pub mod monitor;
#[path = "automation/n8n_api.rs"]
pub mod n8n_api;
#[path = "ops/notifier.rs"]
pub mod notifier;
#[path = "cli_core/peekaboo_cli.rs"]
pub mod peekaboo_cli;
#[cfg(target_os = "macos")]
#[path = "ui_automation/permission_manager.rs"]
pub mod permission_manager;
#[path = "security_core/policy.rs"]
pub mod policy;
pub mod prompts;
#[path = "core_utils/retry_logic.rs"]
pub mod retry_logic;
#[path = "ops/scheduler.rs"]
pub mod scheduler;
#[path = "schemas/schema.rs"]
pub mod schema;
#[path = "state/session.rs"]
pub mod session;
#[path = "state/session_store.rs"]
pub mod session_store;
#[path = "agent/subagent.rs"]
pub mod subagent;
#[path = "ui_automation/tool_chaining.rs"]
pub mod tool_chaining;

pub mod api_server;
#[path = "api_server/types.rs"]
pub mod api_server_types;
#[path = "api/api_dcp.rs"]
pub mod api_dcp;
#[path = "api/api_email_workflow.rs"]
pub mod api_email_workflow;
#[path = "api/api_screen_quality.rs"]
pub mod api_screen_quality;
#[path = "api/api_system.rs"]
pub mod api_system;
#[path = "api/api_utils.rs"]
pub mod api_utils;
#[path = "intelligence/chat_sanitize.rs"]
pub mod chat_sanitize;
#[path = "intelligence/feedback_collector.rs"]
pub mod feedback_collector;
pub mod integrations;
#[path = "analysis_core/memory.rs"]
pub mod memory;
#[path = "runtime_core/main_startup.rs"]
pub mod main_startup;
#[path = "runtime_core/orchestrator.rs"]
pub mod orchestrator;
#[path = "intelligence/pattern_detector.rs"]
pub mod pattern_detector;
#[path = "core_utils/paths.rs"]
pub mod paths;
#[path = "security_core/privacy.rs"]
pub mod privacy;
#[path = "intelligence/recommendation.rs"]
pub mod recommendation;
#[path = "security_core/security.rs"]
pub mod security;
#[path = "security_core/send_policy.rs"]
pub mod send_policy;
#[path = "automation/shell_actions.rs"]
pub mod shell_actions;
#[path = "automation/shell_analysis.rs"]
pub mod shell_analysis;
#[path = "ui_automation/visual_driver.rs"]
pub mod visual_driver;
#[path = "schemas/workflow_schema.rs"]
pub mod workflow_schema;

#[path = "automation/command_queue.rs"]
pub mod command_queue;
#[path = "intelligence/context_pruning.rs"]
pub mod context_pruning;
#[path = "intelligence/project_scanner.rs"]
pub mod project_scanner;
#[path = "verification/runtime_verification.rs"]
pub mod runtime_verification;
#[path = "automation/tool_policy.rs"]
pub mod tool_policy;

#[path = "agent/chat_gate.rs"]
pub mod chat_gate;
#[path = "verification/consistency_check.rs"]
pub mod consistency_check;
#[path = "verification/judgment.rs"]
pub mod judgment;
#[path = "verification/performance_verification.rs"]
pub mod performance_verification;
#[path = "verification/quality_scorer.rs"]
pub mod quality_scorer;
#[path = "verification/release_gate.rs"]
pub mod release_gate;
#[path = "runtime_core/runtime_mode.rs"]
pub mod runtime_mode;
#[path = "verification/semantic_verification.rs"]
pub mod semantic_verification;
#[path = "ops/singleton_lock.rs"]
pub mod singleton_lock;
#[path = "verification/static_checks.rs"]
pub mod static_checks;
#[path = "verification/tool_result_guard.rs"]
pub mod tool_result_guard;
#[path = "verification/visual_verification.rs"]
pub mod visual_verification;

#[path = "agent/execution_controller.rs"]
pub mod execution_controller;
#[path = "agent/intent_router.rs"]
pub mod intent_router;
#[path = "agent/nl_automation.rs"]
pub mod nl_automation;
#[path = "agent/nl_store.rs"]
pub mod nl_store;
#[path = "agent/plan_builder.rs"]
pub mod plan_builder;
#[path = "agent/slot_filler.rs"]
pub mod slot_filler;
#[path = "agent/verification_engine.rs"]
pub mod verification_engine;

pub mod error;

#[path = "cli_core/cli_llm.rs"]
pub mod cli_llm;
#[path = "config/config_manager.rs"]
pub mod config_manager;
#[path = "analysis_core/content_extractor.rs"]
pub mod content_extractor;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
#[path = "analysis_core/reality_check.rs"]
pub mod reality_check;
#[path = "integration_core/collector_bridge.rs"]
pub mod collector_bridge;
#[path = "runtime_core/dynamic_controller.rs"]
pub mod dynamic_controller;
#[path = "runtime_core/executor.rs"]
pub mod executor;
#[path = "planning/replan_templates.rs"]
pub mod replan_templates;
#[path = "planning/replanning_config.rs"]
pub mod replanning_config;
#[path = "ui_automation/screen_recorder.rs"]
pub mod screen_recorder;
#[path = "integration_core/telegram.rs"]
pub mod telegram;

pub fn env_flag(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub fn load_env() {
    let _ = dotenv::dotenv();

    let mut candidates = Vec::new();
    candidates.push(std::path::PathBuf::from("core/.env"));

    if let Ok(cwd) = std::env::current_dir() {
        if let Some(parent) = cwd.parent() {
            candidates.push(parent.join(".env"));
            candidates.push(parent.join("core").join(".env"));
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join(".env"));
            candidates.push(parent.join("core").join(".env"));
        }
    }

    for path in candidates {
        if path.exists() {
            let _ = dotenv::from_path(path);
        }
    }
}
