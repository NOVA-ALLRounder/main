// Prometheus Metrics for monitoring

use anyhow::Result;
use lazy_static::lazy_static;
use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounter,
    IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
};

lazy_static! {
    /// Global metrics registry
    pub static ref METRICS_REGISTRY: Registry = Registry::new();

    // ====================
    // Command Metrics
    // ====================

    /// Total number of commands received
    pub static ref COMMANDS_TOTAL: IntCounter = IntCounter::new(
        "steer_commands_total",
        "Total number of commands received"
    )
    .expect("metric can be created");

    /// Commands by status (success/failure)
    pub static ref COMMANDS_BY_STATUS: IntCounterVec = IntCounterVec::new(
        Opts::new("steer_commands_by_status", "Commands by status"),
        &["status"]
    )
    .expect("metric can be created");

    /// Command execution duration
    pub static ref COMMAND_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("steer_command_duration_seconds", "Command execution duration")
            .buckets(vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0]),
        &["command_type"]
    )
    .expect("metric can be created");

    // ====================
    // Skill Metrics
    // ====================

    /// Skill executions by skill and action
    pub static ref SKILL_EXECUTIONS: IntCounterVec = IntCounterVec::new(
        Opts::new("steer_skill_executions_total", "Skill executions"),
        &["skill", "action", "status"]
    )
    .expect("metric can be created");

    /// Skill execution duration
    pub static ref SKILL_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("steer_skill_duration_seconds", "Skill execution duration")
            .buckets(vec![0.01, 0.1, 0.5, 1.0, 5.0, 10.0]),
        &["skill", "action"]
    )
    .expect("metric can be created");

    // ====================
    // LLM Metrics
    // ====================

    /// LLM API calls
    pub static ref LLM_CALLS: IntCounterVec = IntCounterVec::new(
        Opts::new("steer_llm_calls_total", "LLM API calls"),
        &["model", "status"]
    )
    .expect("metric can be created");

    /// LLM latency
    pub static ref LLM_LATENCY: HistogramVec = HistogramVec::new(
        HistogramOpts::new("steer_llm_latency_seconds", "LLM API latency")
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]),
        &["model"]
    )
    .expect("metric can be created");

    /// LLM tokens used
    pub static ref LLM_TOKENS: IntCounterVec = IntCounterVec::new(
        Opts::new("steer_llm_tokens_total", "LLM tokens consumed"),
        &["model", "type"] // type: prompt/completion
    )
    .expect("metric can be created");

    // ====================
    // Policy Metrics
    // ====================

    /// Policy checks
    pub static ref POLICY_CHECKS: IntCounterVec = IntCounterVec::new(
        Opts::new("steer_policy_checks_total", "Policy checks"),
        &["policy", "result"] // result: allowed/blocked
    )
    .expect("metric can be created");

    /// Approval requests
    pub static ref APPROVAL_REQUESTS: IntCounterVec = IntCounterVec::new(
        Opts::new("steer_approval_requests_total", "Approval requests"),
        &["action", "decision"] // decision: approved/rejected/pending
    )
    .expect("metric can be created");

    // ====================
    // Event Metrics
    // ====================

    /// Events processed
    pub static ref EVENTS_PROCESSED: IntCounterVec = IntCounterVec::new(
        Opts::new("steer_events_processed_total", "Events processed"),
        &["event_type", "source"]
    )
    .expect("metric can be created");

    /// Event queue depth
    pub static ref EVENT_QUEUE_DEPTH: IntGauge = IntGauge::new(
        "steer_event_queue_depth",
        "Current event queue depth"
    )
    .expect("metric can be created");

    // ====================
    // System Metrics
    // ====================

    /// Active sessions
    pub static ref ACTIVE_SESSIONS: IntGauge = IntGauge::new(
        "steer_active_sessions",
        "Number of active user sessions"
    )
    .expect("metric can be created");

    /// System health status (0=unhealthy, 1=healthy)
    pub static ref SYSTEM_HEALTH: IntGaugeVec = IntGaugeVec::new(
        Opts::new("steer_system_health", "System health status"),
        &["component"]
    )
    .expect("metric can be created");

    // ====================
    // Performance Metrics
    // ====================

    /// Parallel execution speedup
    pub static ref PARALLEL_SPEEDUP: Histogram = Histogram::with_opts(
        HistogramOpts::new("steer_parallel_speedup_ratio", "Parallel execution speedup ratio")
            .buckets(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0])
    )
    .expect("metric can be created");

    /// Database query duration
    pub static ref DB_QUERY_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("steer_db_query_duration_seconds", "Database query duration")
            .buckets(vec![0.001, 0.01, 0.1, 0.5, 1.0]),
        &["operation"]
    )
    .expect("metric can be created");
}

/// Initialize metrics (register with Prometheus)
pub fn init_metrics(service_name: &str) -> Result<()> {
    // Register all metrics
    METRICS_REGISTRY
        .register(Box::new(COMMANDS_TOTAL.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(COMMANDS_BY_STATUS.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(COMMAND_DURATION.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(SKILL_EXECUTIONS.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(SKILL_DURATION.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(LLM_CALLS.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(LLM_LATENCY.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(LLM_TOKENS.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(POLICY_CHECKS.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(APPROVAL_REQUESTS.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(EVENTS_PROCESSED.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(EVENT_QUEUE_DEPTH.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(ACTIVE_SESSIONS.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(SYSTEM_HEALTH.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(PARALLEL_SPEEDUP.clone()))
        .ok();
    METRICS_REGISTRY
        .register(Box::new(DB_QUERY_DURATION.clone()))
        .ok();

    log::info!("?諭?Registered {} metrics for '{}'", 16, service_name);
    Ok(())
}

/// Get Prometheus metrics in text format
pub fn gather_metrics() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = METRICS_REGISTRY.gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

// Convenience macros for common metric operations

#[macro_export]
macro_rules! record_command {
    ($status:expr) => {
        $crate::shared::telemetry::metrics::COMMANDS_TOTAL.inc();
        $crate::shared::telemetry::metrics::COMMANDS_BY_STATUS
            .with_label_values(&[$status])
            .inc();
    };
}

#[macro_export]
macro_rules! time_command {
    ($command_type:expr, $code:block) => {{
        let timer = $crate::shared::telemetry::metrics::COMMAND_DURATION
            .with_label_values(&[$command_type])
            .start_timer();
        let result = $code;
        timer.observe_duration();
        result
    }};
}

#[macro_export]
macro_rules! record_skill_execution {
    ($skill:expr, $action:expr, $status:expr) => {
        $crate::shared::telemetry::metrics::SKILL_EXECUTIONS
            .with_label_values(&[$skill, $action, $status])
            .inc();
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let result = init_metrics("test-service");
        assert!(result.is_ok());
    }

    #[test]
    fn test_metrics_gathering() {
        init_metrics("test").ok();
        COMMANDS_TOTAL.inc();
        let output = gather_metrics();
        assert!(output.contains("steer_commands_total"));
    }

    #[test]
    fn test_command_metrics() {
        COMMANDS_TOTAL.inc();
        COMMANDS_BY_STATUS.with_label_values(&["success"]).inc();
        assert_eq!(COMMANDS_TOTAL.get(), 1);
    }
}
