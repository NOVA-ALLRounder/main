// Distributed Tracing with OpenTelemetry

use anyhow::Result;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    runtime,
    trace::{self, RandomIdGenerator, Sampler},
    Resource,
};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use std::time::Duration;

/// Initialize OpenTelemetry tracer
pub fn init_tracer(service_name: &str) -> Result<()> {
    // Check if OTEL_EXPORTER_OTLP_ENDPOINT is set
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

    // Create resource with service information
    let resource = Resource::new(vec![
        KeyValue::new(SERVICE_NAME, service_name.to_string()),
        KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
        KeyValue::new("deployment.environment", get_environment()),
    ]);

    // Configure OTLP exporter
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&endpoint)
        .with_timeout(Duration::from_secs(3));

    // Build tracer provider and install (sets global tracer provider automatically)
    let _tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .with_trace_config(
            trace::config()
                .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                    get_sample_rate(),
                ))))
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(resource),
        )
        .install_batch(runtime::Tokio)?;

    log::info!(
        "?諭?OpenTelemetry exporting to: {} (sample rate: {})",
        endpoint,
        get_sample_rate()
    );

    Ok(())
}

/// Get current environment (dev/staging/production)
fn get_environment() -> String {
    std::env::var("STEER_ENV").unwrap_or_else(|_| "development".to_string())
}

/// Get trace sampling rate from environment
fn get_sample_rate() -> f64 {
    std::env::var("OTEL_TRACE_SAMPLE_RATE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0) // Default: sample all traces
}

/// Create a span for a function
#[macro_export]
macro_rules! trace_span {
    ($name:expr) => {{
        use opentelemetry::trace::Tracer;
        opentelemetry::global::tracer("steer").start($name)
    }};
    ($name:expr, $($key:expr => $value:expr),+) => {{
        use opentelemetry::{trace::Tracer, KeyValue};
        let mut span = opentelemetry::global::tracer("steer").start($name);
        $(
            span.set_attribute(KeyValue::new($key, $value));
        )+
        span
    }};
}

/// Add an event to the current span
#[macro_export]
macro_rules! trace_event {
    ($name:expr) => {{
        use opentelemetry::trace::get_active_span;
        get_active_span(|span| {
            span.add_event($name, vec![]);
        });
    }};
    ($name:expr, $($key:expr => $value:expr),+) => {{
        use opentelemetry::{trace::get_active_span, KeyValue};
        let attributes = vec![
            $(KeyValue::new($key, $value)),+
        ];
        get_active_span(|span| {
            span.add_event($name, attributes);
        });
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_detection() {
        std::env::remove_var("STEER_ENV");
        assert_eq!(get_environment(), "development");

        std::env::set_var("STEER_ENV", "production");
        assert_eq!(get_environment(), "production");
        std::env::remove_var("STEER_ENV");
    }

    #[test]
    fn test_sample_rate() {
        std::env::remove_var("OTEL_TRACE_SAMPLE_RATE");
        assert_eq!(get_sample_rate(), 1.0);

        std::env::set_var("OTEL_TRACE_SAMPLE_RATE", "0.5");
        assert_eq!(get_sample_rate(), 0.5);
        std::env::remove_var("OTEL_TRACE_SAMPLE_RATE");
    }
}
