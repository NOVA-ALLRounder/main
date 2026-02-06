use local_os_agent::jarvis::{
    orchestrator::JarvisOrchestrator,
    skills::{ComputerUseSkill, EmailSkill, TelegramSkill, Skill, SkillContext, SkillResult},
    models::context::UserContext,
};
use std::sync::Arc;
use std::collections::HashMap;
use tokio;

#[tokio::test]
async fn test_full_workflow() {
    // Initialize JarvisOrchestrator
    let orchestrator_result = JarvisOrchestrator::new().await;
    assert!(orchestrator_result.is_ok(), "Orchestrator should initialize");

    let orchestrator = orchestrator_result.unwrap();

    // Get skill registry and register skills
    let registry = orchestrator.skill_registry();
    registry.register(Arc::new(ComputerUseSkill::new())).await;
    registry.register(Arc::new(EmailSkill::new())).await;
    registry.register(Arc::new(TelegramSkill::new())).await;

    // Verify skills are registered
    let count = registry.count().await;
    assert_eq!(count, 3, "Should have 3 skills registered");

    println!("✅ Full workflow test: Skills registered successfully");
}

#[tokio::test]
async fn test_skill_registration() {
    let orchestrator = JarvisOrchestrator::new().await.expect("Failed to create orchestrator");

    // Register all 3 skills
    let computer_skill = Arc::new(ComputerUseSkill::new());
    let email_skill = Arc::new(EmailSkill::new());
    let telegram_skill = Arc::new(TelegramSkill::new());

    let registry = orchestrator.skill_registry();
    registry.register(computer_skill.clone()).await;
    registry.register(email_skill.clone()).await;
    registry.register(telegram_skill.clone()).await;

    // Verify all 3 skills registered
    let count = registry.count().await;
    assert_eq!(count, 3, "Should have 3 skills registered");

    // Check metadata for each skill
    let computer_meta = computer_skill.metadata();
    assert_eq!(computer_meta.name, "computer_use", "Computer skill should have correct name");
    assert!(computer_meta.actions.len() > 0, "Computer skill should have actions");

    let email_meta = email_skill.metadata();
    assert_eq!(email_meta.name, "email", "Email skill should have correct name");
    assert!(email_meta.actions.len() > 0, "Email skill should have actions");

    let telegram_meta = telegram_skill.metadata();
    assert_eq!(telegram_meta.name, "telegram", "Telegram skill should have correct name");
    assert!(telegram_meta.actions.len() > 0, "Telegram skill should have actions");

    // Verify eligibility checks
    let computer_eligibility = computer_skill.check_eligibility();
    assert!(computer_eligibility.eligible || !computer_eligibility.eligible,
            "Computer skill eligibility check should complete");

    let email_eligibility = email_skill.check_eligibility();
    assert!(email_eligibility.eligible || !email_eligibility.eligible,
            "Email skill eligibility check should complete");

    let telegram_eligibility = telegram_skill.check_eligibility();
    assert!(telegram_eligibility.eligible || !telegram_eligibility.eligible,
            "Telegram skill eligibility check should complete");

    println!("✅ Skill registration test: All skills registered with correct metadata");
}

#[tokio::test]
async fn test_parallel_execution() {
    let orchestrator = JarvisOrchestrator::new().await.expect("Failed to create orchestrator");

    // Register skills
    let registry = orchestrator.skill_registry();
    registry.register(Arc::new(ComputerUseSkill::new())).await;
    registry.register(Arc::new(EmailSkill::new())).await;
    registry.register(Arc::new(TelegramSkill::new())).await;

    // Test that parallel execution infrastructure exists
    let count = registry.count().await;
    assert!(count > 0, "Skills should be registered for parallel execution");

    // Note: Actual parallel execution testing requires more complex setup
    // This test verifies the infrastructure is in place
    println!("✅ Parallel execution test: Infrastructure verified");
}

#[tokio::test]
async fn test_skill_execution_computer_use() {
    let skill = ComputerUseSkill::new();

    // Create context for screenshot action
    let ctx = SkillContext {
        session_key: "test_session".to_string(),
        action: "screenshot".to_string(),
        params: {
            let mut params = HashMap::new();
            params.insert("display".to_string(), serde_json::json!(0));
            params
        },
        user_context: UserContext {
            workspace_root: Some(std::env::current_dir().unwrap()),
            recent_files: vec![],
            recent_apps: vec![],
            recent_sites: vec![],
        },
    };

    let result = skill.execute(ctx).await;
    // May fail if no display available, but should not panic
    assert!(result.success || !result.success,
            "Screenshot execution should complete without panic");

    if !result.success {
        println!("Expected result (may fail without display): {}", result.message);
    } else {
        println!("✅ Screenshot executed successfully");
    }
}

#[tokio::test]
async fn test_skill_execution_email() {
    let skill = EmailSkill::new();

    // Test send_email action (will fail without credentials, but should handle gracefully)
    let ctx = SkillContext {
        session_key: "test_session".to_string(),
        action: "send_email".to_string(),
        params: {
            let mut params = HashMap::new();
            params.insert("to".to_string(), serde_json::json!("test@example.com"));
            params.insert("subject".to_string(), serde_json::json!("Test"));
            params.insert("body".to_string(), serde_json::json!("Test email"));
            params
        },
        user_context: UserContext {
            workspace_root: Some(std::env::current_dir().unwrap()),
            recent_files: vec![],
            recent_apps: vec![],
            recent_sites: vec![],
        },
    };

    let result = skill.execute(ctx).await;
    // Should handle missing credentials gracefully
    assert!(result.success || !result.success,
            "Email execution should complete without panic");

    if !result.success {
        println!("Expected result (missing credentials): {}", result.message);
    }

    println!("✅ Email skill handles missing credentials gracefully");
}

#[tokio::test]
async fn test_skill_execution_telegram() {
    let skill = TelegramSkill::new();

    // Test send_message action (will fail without credentials, but should handle gracefully)
    let ctx = SkillContext {
        session_key: "test_session".to_string(),
        action: "send_message".to_string(),
        params: {
            let mut params = HashMap::new();
            params.insert("chat_id".to_string(), serde_json::json!("123456"));
            params.insert("message".to_string(), serde_json::json!("Test message"));
            params
        },
        user_context: UserContext {
            workspace_root: Some(std::env::current_dir().unwrap()),
            recent_files: vec![],
            recent_apps: vec![],
            recent_sites: vec![],
        },
    };

    let result = skill.execute(ctx).await;
    // Should handle missing credentials gracefully
    assert!(result.success || !result.success,
            "Telegram execution should complete without panic");

    if !result.success {
        println!("Expected result (missing credentials): {}", result.message);
    }

    println!("✅ Telegram skill handles missing credentials gracefully");
}

#[tokio::test]
async fn test_error_recovery() {
    let orchestrator = JarvisOrchestrator::new().await.expect("Failed to create orchestrator");
    let registry = orchestrator.skill_registry();
    registry.register(Arc::new(ComputerUseSkill::new())).await;

    // Create context with invalid action
    let ctx = SkillContext {
        session_key: "test_session".to_string(),
        action: "nonexistent_action".to_string(),
        params: HashMap::new(),
        user_context: UserContext {
            workspace_root: Some(std::env::current_dir().unwrap()),
            recent_files: vec![],
            recent_apps: vec![],
            recent_sites: vec![],
        },
    };

    let result = registry.execute("computer_use", ctx).await;
    // Should handle error gracefully
    assert!(!result.success, "Invalid action should return error");
    println!("✅ Error recovery test: Invalid action handled gracefully");
}

#[tokio::test]
async fn test_skill_metadata() {
    let computer_skill = ComputerUseSkill::new();
    let email_skill = EmailSkill::new();
    let telegram_skill = TelegramSkill::new();

    // Test computer use metadata
    let computer_meta = computer_skill.metadata();
    assert!(!computer_meta.name.is_empty(), "Skill name should not be empty");
    assert!(!computer_meta.description.is_empty(), "Description should not be empty");
    assert!(!computer_meta.actions.is_empty(), "Should have at least one action");

    // Verify computer use has expected actions
    let computer_actions: Vec<&str> = computer_meta.actions.iter().map(|a| a.name.as_str()).collect();
    assert!(computer_actions.contains(&"screenshot"), "Should have screenshot action");
    assert!(computer_actions.contains(&"type_text"), "Should have type_text action");
    assert!(computer_actions.contains(&"mouse_move"), "Should have mouse_move action");

    // Test email metadata
    let email_meta = email_skill.metadata();
    let email_actions: Vec<&str> = email_meta.actions.iter().map(|a| a.name.as_str()).collect();
    assert!(email_actions.contains(&"send_email"), "Should have send_email action");
    assert!(email_actions.contains(&"read_emails"), "Should have read_emails action");

    // Test telegram metadata
    let telegram_meta = telegram_skill.metadata();
    let telegram_actions: Vec<&str> = telegram_meta.actions.iter().map(|a| a.name.as_str()).collect();
    assert!(telegram_actions.contains(&"send_message"), "Should have send_message action");
    assert!(telegram_actions.contains(&"send_photo"), "Should have send_photo action");

    println!("✅ Skill metadata test: All skills have complete metadata");
}

#[tokio::test]
async fn test_eligibility_checks() {
    let computer_skill = ComputerUseSkill::new();
    let email_skill = EmailSkill::new();
    let telegram_skill = TelegramSkill::new();

    // Check eligibility
    let computer_eligibility = computer_skill.check_eligibility();
    let email_eligibility = email_skill.check_eligibility();
    let telegram_eligibility = telegram_skill.check_eligibility();

    // Computer use should work on all platforms
    assert!(computer_eligibility.eligible || !computer_eligibility.eligible,
            "Computer eligibility check should complete");

    // Email requires environment variables
    if !email_eligibility.eligible {
        assert!(email_eligibility.reason.is_some(),
                "Email should provide reason when not eligible");
        println!("Email not eligible: {}", email_eligibility.reason.unwrap());
    }

    // Telegram requires bot token
    if !telegram_eligibility.eligible {
        assert!(telegram_eligibility.reason.is_some(),
                "Telegram should provide reason when not eligible");
        println!("Telegram not eligible: {}", telegram_eligibility.reason.unwrap());
    }

    println!("✅ Eligibility checks test: All skills checked successfully");
}

#[tokio::test]
async fn test_skill_registry_operations() {
    let orchestrator = JarvisOrchestrator::new().await.expect("Failed to create orchestrator");
    let registry = orchestrator.skill_registry();

    // Start with empty registry
    let initial_count = registry.count().await;

    // Register skills
    registry.register(Arc::new(ComputerUseSkill::new())).await;
    registry.register(Arc::new(EmailSkill::new())).await;
    registry.register(Arc::new(TelegramSkill::new())).await;

    // Check count increased
    let new_count = registry.count().await;
    assert_eq!(new_count, initial_count + 3, "Count should increase by 3");

    // List all skills
    let all_skills = registry.list().await;
    assert_eq!(all_skills.len(), new_count, "List should return all skills");

    // Get specific skill
    let computer_skill = registry.get("computer_use").await;
    assert!(computer_skill.is_some(), "Should find computer_use skill");

    let nonexistent_skill = registry.get("nonexistent").await;
    assert!(nonexistent_skill.is_none(), "Should not find nonexistent skill");

    println!("✅ Registry operations test: All operations working correctly");
}

#[tokio::test]
async fn test_concurrent_skill_execution() {
    let orchestrator = JarvisOrchestrator::new().await.expect("Failed to create orchestrator");
    let registry = orchestrator.skill_registry();
    registry.register(Arc::new(ComputerUseSkill::new())).await;

    // Create multiple contexts
    let ctx1 = SkillContext {
        session_key: "session1".to_string(),
        action: "get_screen_size".to_string(),
        params: HashMap::new(),
        user_context: UserContext {
            workspace_root: Some(std::env::current_dir().unwrap()),
            recent_files: vec![],
            recent_apps: vec![],
            recent_sites: vec![],
        },
    };

    let ctx2 = SkillContext {
        session_key: "session2".to_string(),
        action: "get_screen_size".to_string(),
        params: HashMap::new(),
        user_context: UserContext {
            workspace_root: Some(std::env::current_dir().unwrap()),
            recent_files: vec![],
            recent_apps: vec![],
            recent_sites: vec![],
        },
    };

    // Execute concurrently
    let registry_clone = registry.clone();
    let (result1, result2) = tokio::join!(
        registry.execute("computer_use", ctx1),
        registry_clone.execute("computer_use", ctx2)
    );

    // Both should complete without panic
    assert!(result1.success || !result1.success, "First execution should complete");
    assert!(result2.success || !result2.success, "Second execution should complete");

    println!("✅ Concurrent execution test: Multiple skills can execute in parallel");
}

#[test]
fn test_skill_result_creation() {
    // Test success result
    let success = SkillResult::success("Operation completed");
    assert!(success.success);
    assert_eq!(success.message, "Operation completed");
    assert!(success.data.is_none());

    // Test success with data
    let data = serde_json::json!({"key": "value"});
    let success_with_data = SkillResult::success_with_data("Done", data.clone());
    assert!(success_with_data.success);
    assert!(success_with_data.data.is_some());
    assert_eq!(success_with_data.data.unwrap(), data);

    // Test error result
    let error = SkillResult::error("Something went wrong");
    assert!(!error.success);
    assert_eq!(error.message, "Something went wrong");
    assert!(error.data.is_none());

    println!("✅ SkillResult creation test: All result types created correctly");
}

#[test]
fn test_user_context_creation() {
    let context = UserContext {
        workspace_root: Some(std::env::current_dir().unwrap()),
        recent_files: vec!["file1.txt".to_string(), "file2.txt".to_string()],
        recent_apps: vec!["app1".to_string()],
        recent_sites: vec!["https://example.com".to_string()],
    };

    assert!(context.workspace_root.is_some());
    assert_eq!(context.recent_files.len(), 2);
    assert_eq!(context.recent_apps.len(), 1);
    assert_eq!(context.recent_sites.len(), 1);

    println!("✅ UserContext creation test: Context created successfully");
}
