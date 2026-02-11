// Error Handler - Retry logic and error notification

use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;

/// Error classification
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorSeverity {
    /// Transient error - can be retried
    Transient,
    /// Permanent error - should not retry
    Permanent,
    /// Critical error - requires immediate attention
    Critical,
}

/// Error classification result
#[derive(Debug)]
pub struct ErrorClassification {
    pub severity: ErrorSeverity,
    pub should_notify: bool,
    pub should_retry: bool,
    pub max_retries: u32,
}

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            multiplier: 2.0,
        }
    }
}

/// Classify error to determine retry strategy
pub fn classify_error(error: &anyhow::Error) -> ErrorClassification {
    let error_str = error.to_string().to_lowercase();

    // Check for transient errors
    if error_str.contains("timeout")
        || error_str.contains("connection refused")
        || error_str.contains("network")
        || error_str.contains("temporary")
    {
        return ErrorClassification {
            severity: ErrorSeverity::Transient,
            should_notify: false,
            should_retry: true,
            max_retries: 3,
        };
    }

    // Check for critical errors
    if error_str.contains("panic")
        || error_str.contains("fatal")
        || error_str.contains("critical")
        || error_str.contains("segmentation fault")
    {
        return ErrorClassification {
            severity: ErrorSeverity::Critical,
            should_notify: true,
            should_retry: false,
            max_retries: 0,
        };
    }

    // Check for permanent errors
    if error_str.contains("not found")
        || error_str.contains("permission denied")
        || error_str.contains("invalid")
        || error_str.contains("unauthorized")
    {
        return ErrorClassification {
            severity: ErrorSeverity::Permanent,
            should_notify: true,
            should_retry: false,
            max_retries: 0,
        };
    }

    // Default: treat as transient
    ErrorClassification {
        severity: ErrorSeverity::Transient,
        should_notify: false,
        should_retry: true,
        max_retries: 2,
    }
}

/// Execute a function with retry logic
pub async fn with_retry<F, Fut, T>(
    operation: F,
    config: RetryConfig,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempt = 0;
    let mut delay_ms = config.initial_delay_ms;

    loop {
        attempt += 1;

        match operation().await {
            Ok(result) => {
                if attempt > 1 {
                    log::info!("Operation succeeded after {} attempts", attempt);
                }
                return Ok(result);
            }
            Err(e) => {
                let classification = classify_error(&e);

                // Check if we should retry
                if !classification.should_retry || attempt >= config.max_attempts {
                    log::error!(
                        "Operation failed after {} attempts: {} (severity: {:?})",
                        attempt,
                        e,
                        classification.severity
                    );
                    return Err(e);
                }

                // Log retry attempt
                log::warn!(
                    "Attempt {}/{} failed: {}. Retrying in {}ms...",
                    attempt,
                    config.max_attempts,
                    e,
                    delay_ms
                );

                // Wait before retry with exponential backoff
                sleep(Duration::from_millis(delay_ms)).await;

                // Increase delay for next attempt (exponential backoff)
                delay_ms = (delay_ms as f64 * config.multiplier) as u64;
                delay_ms = delay_ms.min(config.max_delay_ms);
            }
        }
    }
}

/// Error handler for JARVIS orchestrator
pub struct ErrorHandler {
    telegram_enabled: bool,
}

impl ErrorHandler {
    pub fn new() -> Self {
        Self {
            telegram_enabled: std::env::var("TELEGRAM_BOT_TOKEN").is_ok(),
        }
    }

    /// Handle error with appropriate actions (logging, notification, etc.)
    pub async fn handle_error(&self, error: &anyhow::Error, context: &str) -> ErrorClassification {
        let classification = classify_error(error);

        // Log the error
        match classification.severity {
            ErrorSeverity::Critical => {
                log::error!("???CRITICAL ERROR in {}: {}", context, error);
            }
            ErrorSeverity::Permanent => {
                log::error!("??Permanent error in {}: {}", context, error);
            }
            ErrorSeverity::Transient => {
                log::warn!("??ル쵑?? Transient error in {}: {}", context, error);
            }
        }

        // Send Telegram notification if enabled and needed
        if classification.should_notify && self.telegram_enabled {
            if let Err(e) = self.send_error_notification(error, context, &classification).await {
                log::error!("Failed to send error notification: {}", e);
            }
        }

        classification
    }

    /// Send error notification via Telegram
    async fn send_error_notification(
        &self,
        error: &anyhow::Error,
        context: &str,
        classification: &ErrorClassification,
    ) -> Result<()> {
        let token = std::env::var("TELEGRAM_BOT_TOKEN")?;
        let chat_id = std::env::var("TELEGRAM_USER_ID")?.parse::<i64>()?;

        let severity_emoji = match classification.severity {
            ErrorSeverity::Critical => "[CRIT]",
            ErrorSeverity::Permanent => "[ERR]",
            ErrorSeverity::Transient => "[WARN]",
        };

        let message = format!(
            "{} *JARVIS Error*\n\n*Context:* {}\n*Severity:* {:?}\n*Error:* {}\n\n_{}_ UTC",
            severity_emoji,
            context,
            classification.severity,
            error,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
        );

        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
        let body = serde_json::json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "Markdown"
        });

        reqwest::Client::new()
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        log::info!("Error notification sent to Telegram");
        Ok(())
    }
}

impl Default for ErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_classification_transient() {
        let error = anyhow::anyhow!("Connection timeout");
        let classification = classify_error(&error);

        assert_eq!(classification.severity, ErrorSeverity::Transient);
        assert!(classification.should_retry);
        assert!(!classification.should_notify);
    }

    #[test]
    fn test_error_classification_permanent() {
        let error = anyhow::anyhow!("File not found");
        let classification = classify_error(&error);

        assert_eq!(classification.severity, ErrorSeverity::Permanent);
        assert!(!classification.should_retry);
        assert!(classification.should_notify);
    }

    #[test]
    fn test_error_classification_critical() {
        let error = anyhow::anyhow!("Fatal error: panic");
        let classification = classify_error(&error);

        assert_eq!(classification.severity, ErrorSeverity::Critical);
        assert!(!classification.should_retry);
        assert!(classification.should_notify);
    }

    #[tokio::test]
    async fn test_retry_success() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let operation = move || {
            let attempts = attempts_clone.clone();
            async move {
                let count = attempts.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(anyhow::anyhow!("Temporary failure"))
                } else {
                    Ok(42)
                }
            }
        };

        let config = RetryConfig {
            max_attempts: 5,
            initial_delay_ms: 10,
            max_delay_ms: 100,
            multiplier: 2.0,
        };

        let result = with_retry(operation, config).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_max_attempts() {
        let operation = || async { Err(anyhow::anyhow!("Temporary failure")) };

        let config = RetryConfig {
            max_attempts: 2,
            initial_delay_ms: 10,
            max_delay_ms: 100,
            multiplier: 2.0,
        };

        let result: Result<i32> = with_retry(operation, config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_error_handler_creation() {
        let handler = ErrorHandler::new();
        assert!(!handler.telegram_enabled || handler.telegram_enabled);
    }
}
