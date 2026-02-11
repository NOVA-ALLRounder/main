// Telemetry - OpenTelemetry integration for distributed tracing and metrics

pub mod tracing;
pub mod metrics;

pub use self::tracing::init_tracer;
pub use self::metrics::init_metrics;

use anyhow::Result;

/// Initialize all telemetry systems (tracing + metrics)
pub async fn init_telemetry(service_name: &str) -> Result<()> {
    // Initialize tracing
    init_tracer(service_name)?;
    log::info!("??OpenTelemetry tracing initialized");

    // Initialize metrics
    init_metrics(service_name)?;
    log::info!("??Prometheus metrics initialized");

    Ok(())
}

/// Shutdown telemetry systems gracefully
pub async fn shutdown_telemetry() -> Result<()> {
    // Flush remaining spans
    opentelemetry::global::shutdown_tracer_provider();
    log::info!("?諭?Telemetry shutdown complete");
    Ok(())
}
