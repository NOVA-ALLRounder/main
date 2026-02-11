// Performance Monitoring and Optimization

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Performance metrics for operations
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub operation: String,
    pub duration_ms: u64,
    pub timestamp: Instant,
    pub success: bool,
}

/// Performance monitor with metrics tracking
pub struct PerformanceMonitor {
    metrics: Arc<RwLock<Vec<PerformanceMetrics>>>,
    operation_stats: Arc<RwLock<HashMap<String, OperationStats>>>,
}

#[derive(Debug, Clone, Default)]
pub struct OperationStats {
    count: u64,
    total_duration_ms: u64,
    avg_duration_ms: u64,
    min_duration_ms: u64,
    max_duration_ms: u64,
    success_count: u64,
    failure_count: u64,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Vec::new())),
            operation_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a performance metric
    pub async fn record(&self, operation: String, duration: Duration, success: bool) {
        let duration_ms = duration.as_millis() as u64;

        // Add to metrics history
        let metric = PerformanceMetrics {
            operation: operation.clone(),
            duration_ms,
            timestamp: Instant::now(),
            success,
        };

        {
            let mut metrics = self.metrics.write().await;
            metrics.push(metric);

            // Keep only last 1000 metrics to avoid memory bloat
            if metrics.len() > 1000 {
                metrics.drain(0..100);
            }
        }

        // Update operation stats
        {
            let mut stats = self.operation_stats.write().await;
            let op_stats = stats.entry(operation.clone()).or_default();

            op_stats.count += 1;
            op_stats.total_duration_ms += duration_ms;

            if op_stats.min_duration_ms == 0 || duration_ms < op_stats.min_duration_ms {
                op_stats.min_duration_ms = duration_ms;
            }

            if duration_ms > op_stats.max_duration_ms {
                op_stats.max_duration_ms = duration_ms;
            }

            op_stats.avg_duration_ms = op_stats.total_duration_ms / op_stats.count;

            if success {
                op_stats.success_count += 1;
            } else {
                op_stats.failure_count += 1;
            }
        }

        // Log slow operations (> 2 seconds)
        if duration_ms > 2000 {
            log::warn!(
                "?醫묓닔  Slow operation detected: {} took {}ms (threshold: 2000ms)",
                operation,
                duration_ms
            );
        }
    }

    /// Get statistics for a specific operation
    pub async fn get_stats(&self, operation: &str) -> Option<OperationStats> {
        let stats = self.operation_stats.read().await;
        stats.get(operation).cloned()
    }

    /// Get all operation statistics
    pub async fn get_all_stats(&self) -> HashMap<String, OperationStats> {
        self.operation_stats.read().await.clone()
    }

    /// Get recent metrics (last N)
    pub async fn get_recent_metrics(&self, count: usize) -> Vec<PerformanceMetrics> {
        let metrics = self.metrics.read().await;
        let start = metrics.len().saturating_sub(count);
        metrics[start..].to_vec()
    }

    /// Clear all metrics
    pub async fn clear(&self) {
        self.metrics.write().await.clear();
        self.operation_stats.write().await.clear();
    }

    /// Get performance summary
    pub async fn get_summary(&self) -> PerformanceSummary {
        let stats = self.operation_stats.read().await;
        let metrics = self.metrics.read().await;

        let total_operations = stats.values().map(|s| s.count).sum();
        let total_successes = stats.values().map(|s| s.success_count).sum();
        let total_failures = stats.values().map(|s| s.failure_count).sum();

        let avg_response_time = if !stats.is_empty() {
            stats.values().map(|s| s.avg_duration_ms).sum::<u64>() / stats.len() as u64
        } else {
            0
        };

        // Find slowest operation
        let slowest_operation = stats
            .iter()
            .max_by_key(|(_, s)| s.max_duration_ms)
            .map(|(name, s)| (name.clone(), s.max_duration_ms));

        PerformanceSummary {
            total_operations,
            total_successes,
            total_failures,
            avg_response_time_ms: avg_response_time,
            slowest_operation,
            metrics_count: metrics.len(),
        }
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance summary
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub total_operations: u64,
    pub total_successes: u64,
    pub total_failures: u64,
    pub avg_response_time_ms: u64,
    pub slowest_operation: Option<(String, u64)>,
    pub metrics_count: usize,
}

/// Helper macro to measure operation performance
#[macro_export]
macro_rules! measure_performance {
    ($monitor:expr, $operation:expr, $code:block) => {{
        let start = std::time::Instant::now();
        let result = $code;
        let duration = start.elapsed();
        let success = result.is_ok();
        $monitor.record($operation.to_string(), duration, success).await;
        result
    }};
}

/// Wrapper for async operations with performance tracking
pub async fn track_performance<F, Fut, T>(
    monitor: &PerformanceMonitor,
    operation: &str,
    fut: F,
) -> anyhow::Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let start = Instant::now();
    let result = fut().await;
    let duration = start.elapsed();
    let success = result.is_ok();

    monitor.record(operation.to_string(), duration, success).await;

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_performance_monitor() {
        let monitor = PerformanceMonitor::new();

        // Record some metrics
        monitor
            .record("test_op".to_string(), Duration::from_millis(100), true)
            .await;
        monitor
            .record("test_op".to_string(), Duration::from_millis(200), true)
            .await;
        monitor
            .record("test_op".to_string(), Duration::from_millis(150), false)
            .await;

        // Check stats
        let stats = monitor.get_stats("test_op").await.unwrap();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.success_count, 2);
        assert_eq!(stats.failure_count, 1);
        assert_eq!(stats.min_duration_ms, 100);
        assert_eq!(stats.max_duration_ms, 200);
    }

    #[tokio::test]
    async fn test_track_performance() {
        let monitor = PerformanceMonitor::new();

        let result = track_performance(&monitor, "async_test", || async {
            sleep(Duration::from_millis(50)).await;
            Ok::<_, anyhow::Error>(42)
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let stats = monitor.get_stats("async_test").await.unwrap();
        assert_eq!(stats.count, 1);
        assert_eq!(stats.success_count, 1);
        assert!(stats.avg_duration_ms >= 50);
    }

    #[tokio::test]
    async fn test_performance_summary() {
        let monitor = PerformanceMonitor::new();

        monitor
            .record("op1".to_string(), Duration::from_millis(100), true)
            .await;
        monitor
            .record("op2".to_string(), Duration::from_millis(3000), true)
            .await;
        monitor
            .record("op3".to_string(), Duration::from_millis(50), false)
            .await;

        let summary = monitor.get_summary().await;
        assert_eq!(summary.total_operations, 3);
        assert_eq!(summary.total_successes, 2);
        assert_eq!(summary.total_failures, 1);
        assert!(summary.slowest_operation.is_some());

        let (op_name, duration) = summary.slowest_operation.unwrap();
        assert_eq!(op_name, "op2");
        assert_eq!(duration, 3000);
    }

    #[tokio::test]
    async fn test_metrics_limit() {
        let monitor = PerformanceMonitor::new();

        // Add more than 1000 metrics
        for i in 0..1100 {
            monitor
                .record(
                    format!("op_{}", i % 10),
                    Duration::from_millis(10),
                    true,
                )
                .await;
        }

        let metrics = monitor.get_recent_metrics(2000).await;
        // Should be capped at 1000, but after cleanup should be around 900
        assert!(metrics.len() <= 1000);
    }
}
