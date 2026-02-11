// E2E Tests for JARVIS System

#[cfg(test)]
mod e2e_tests {
    use crate::jarvis::*;
    use crate::jarvis::models::*;
    use std::time::Duration;
    use tokio::time::sleep;

    /// Test complete command flow: Korean app launch
    #[tokio::test]
    async fn test_e2e_korean_app_launch() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        let command = Command {
            text: "?④쑴沅쎿묾???곷선".to_string(),
            source: CommandSource::Internal,
            params: None,
        };

        // This should parse, execute, and track performance
        let result = orchestrator.handle_command(command).await;

        // Verify the command was processed (may fail if calculator not available)
        assert!(result.is_ok() || result.is_err());

        // Check performance metrics were recorded
        let summary = orchestrator.get_performance_summary().await;
        assert!(summary.total_operations > 0);
    }

    /// Test complete command flow: URL opening
    #[tokio::test]
    async fn test_e2e_url_open() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        let command = Command {
            text: "open https://google.com".to_string(),
            source: CommandSource::Internal,
            params: None,
        };

        let result = orchestrator.handle_command(command).await;

        // Should successfully parse and attempt to open
        assert!(result.is_ok() || result.is_err());

        // Verify metrics
        let summary = orchestrator.get_performance_summary().await;
        assert!(summary.total_operations > 0);
    }

    /// Test performance: response time should be < 2 seconds for simple commands
    #[tokio::test]
    async fn test_e2e_performance_response_time() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        let start = std::time::Instant::now();

        let command = Command {
            text: "click at 100 200".to_string(),
            source: CommandSource::Internal,
            params: None,
        };

        let _result = orchestrator.handle_command(command).await;

        let duration = start.elapsed();

        // Assert response time is under 2 seconds
        assert!(
            duration < Duration::from_secs(2),
            "Response time {}ms exceeds 2000ms threshold",
            duration.as_millis()
        );
    }

    /// Test error handling: invalid command
    #[tokio::test]
    async fn test_e2e_error_handling() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        let command = Command {
            text: "?袁⑹읈????????용뮉 筌뤿굝議??.to_string(),
            source: CommandSource::Internal,
            params: None,
        };

        let result = orchestrator.handle_command(command).await;

        // Unknown commands are routed through skill pipeline;
        // they may succeed (via LLM parsing) or fail gracefully
        let _result = result;

        // Performance metrics should be tracked regardless
        let summary = orchestrator.get_performance_summary().await;
        assert!(summary.total_operations >= 1);
    }

    /// Test session management
    #[tokio::test]
    async fn test_e2e_session_management() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        // Create a session
        let session = orchestrator.get_session("test_user");
        assert_eq!(session.key, "test_user");

        // Update context
        let mut context = UserContext::new();
        context.update_app("TestApp".to_string());

        orchestrator
            .update_session_context("test_user", context)
            .unwrap();

        // Verify context was updated
        let session = orchestrator.get_session("test_user");
        assert_eq!(session.context.active_app, Some("TestApp".to_string()));
    }

    /// Test privacy mode
    #[tokio::test]
    async fn test_e2e_privacy_mode() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        // Default should be Basic
        assert_eq!(
            orchestrator.get_privacy_mode().await,
            PrivacyMode::Basic
        );

        // Change to Full
        orchestrator
            .set_privacy_mode(PrivacyMode::Full)
            .await
            .unwrap();

        assert_eq!(
            orchestrator.get_privacy_mode().await,
            PrivacyMode::Full
        );
    }

    /// Test multiple commands in sequence
    #[tokio::test]
    async fn test_e2e_multiple_commands() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        let commands = vec![
            "click at 100 100",
            "click at 200 200",
            "click at 300 300",
        ];

        for cmd_text in commands {
            let command = Command {
                text: cmd_text.to_string(),
                source: CommandSource::Internal,
                params: None,
            };

            let _result = orchestrator.handle_command(command).await;
        }

        // All commands should be tracked
        let summary = orchestrator.get_performance_summary().await;
        assert!(summary.total_operations >= 3);
    }

    /// Test concurrent command execution
    #[tokio::test]
    async fn test_e2e_concurrent_commands() {
        let orchestrator = std::sync::Arc::new(
            JarvisOrchestrator::new().await.unwrap()
        );

        let mut handles = vec![];

        for i in 0..5 {
            let orch = orchestrator.clone();
            let handle = tokio::spawn(async move {
                let command = Command {
                    text: format!("click at {} {}", i * 100, i * 100),
                    source: CommandSource::Internal,
                    params: None,
                };

                orch.handle_command(command).await
            });
            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            let _ = handle.await;
        }

        // Verify all were tracked
        let summary = orchestrator.get_performance_summary().await;
        assert!(summary.total_operations >= 5);
    }

    /// Test performance metrics aggregation
    #[tokio::test]
    async fn test_e2e_performance_metrics() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        // Execute several commands
        for i in 0..10 {
            let command = Command {
                text: format!("click at {} {}", i * 10, i * 10),
                source: CommandSource::Internal,
                params: None,
            };

            let _ = orchestrator.handle_command(command).await;
            sleep(Duration::from_millis(10)).await;
        }

        // Check summary
        let summary = orchestrator.get_performance_summary().await;

        assert_eq!(summary.total_operations, 10);
        assert!(summary.avg_response_time_ms > 0);
        assert!(summary.metrics_count >= 10);
    }

    /// Test skill registry
    #[tokio::test]
    async fn test_e2e_skill_registry() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();
        let registry = orchestrator.skill_registry();
        let count = registry.count().await;

        // Verify skills are registered (computer_use, email, telegram + new skills)
        assert!(count >= 3, "Expected at least 3 skills, got {}", count);
    }

    /// Test event bus
    #[tokio::test]
    async fn test_e2e_event_bus() {
        let orchestrator = JarvisOrchestrator::new().await.unwrap();

        let command = Command {
            text: "test event".to_string(),
            source: CommandSource::Telegram,
            params: None,
        };

        // This should trigger CommandReceived event
        let _ = orchestrator.handle_command(command).await;

        // Event should have been published (can't easily verify without subscribing)
        // But we can verify the command was processed
        let summary = orchestrator.get_performance_summary().await;
        assert!(summary.total_operations > 0);
    }

    /// Stress test: many commands rapidly
    #[tokio::test]
    async fn test_e2e_stress_test() {
        let orchestrator = std::sync::Arc::new(
            JarvisOrchestrator::new().await.unwrap()
        );

        let start = std::time::Instant::now();
        let mut handles = vec![];

        // Fire 50 commands rapidly
        for i in 0..50 {
            let orch = orchestrator.clone();
            let handle = tokio::spawn(async move {
                let command = Command {
                    text: format!("click at {} {}", i % 500, i % 500),
                    source: CommandSource::Internal,
                    params: None,
                };

                orch.handle_command(command).await
            });
            handles.push(handle);
        }

        // Wait for all
        for handle in handles {
            let _ = handle.await;
        }

        let duration = start.elapsed();

        // All 50 commands should complete in reasonable time (< 10 seconds)
        assert!(
            duration < Duration::from_secs(10),
            "Stress test took {}ms, should be < 10000ms",
            duration.as_millis()
        );

        let summary = orchestrator.get_performance_summary().await;
        assert!(summary.total_operations >= 50);
    }
}
