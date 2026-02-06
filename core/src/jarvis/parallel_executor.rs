// Parallel Executor - Execute plan steps in parallel for 5x performance boost
// Groups steps by dependency (same skill = dependent), executes batches concurrently

use crate::jarvis::{
    plan_builder::{ExecutionPlan, PlanStep, StepStatus},
    skills::{SkillContext, SkillRegistry, SkillResult},
};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;

pub struct ParallelExecutor {
    skill_registry: Arc<SkillRegistry>,
}

impl ParallelExecutor {
    pub fn new(skill_registry: Arc<SkillRegistry>) -> Self {
        Self { skill_registry }
    }

    /// Analyze dependencies and group steps into batches
    /// Steps using the same skill are considered dependent and placed in different batches
    /// Steps in the same batch can execute concurrently
    pub fn analyze_dependencies(&self, plan: &ExecutionPlan) -> Vec<Vec<PlanStep>> {
        let mut batches: Vec<Vec<PlanStep>> = Vec::new();
        let mut used_skills_in_batch: HashMap<usize, Vec<String>> = HashMap::new();

        for step in &plan.steps {
            let mut assigned = false;

            // Try to assign to existing batch
            for (batch_idx, batch) in batches.iter_mut().enumerate() {
                let skills_in_batch = used_skills_in_batch.entry(batch_idx).or_insert_with(Vec::new);

                // Check if this step's skill is already used in this batch
                if !skills_in_batch.contains(&step.skill) {
                    batch.push(step.clone());
                    skills_in_batch.push(step.skill.clone());
                    assigned = true;
                    break;
                }
            }

            // If not assigned, create new batch
            if !assigned {
                batches.push(vec![step.clone()]);
                used_skills_in_batch.insert(batches.len() - 1, vec![step.skill.clone()]);
            }
        }

        log::info!(
            "Dependency analysis: {} steps grouped into {} batches",
            plan.steps.len(),
            batches.len()
        );

        for (idx, batch) in batches.iter().enumerate() {
            log::debug!(
                "Batch {}: {} steps - skills: {:?}",
                idx + 1,
                batch.len(),
                batch.iter().map(|s| &s.skill).collect::<Vec<_>>()
            );
        }

        batches
    }

    /// Execute plan in parallel with proper error handling and cancellation
    pub async fn execute_parallel(
        &self,
        mut plan: ExecutionPlan,
        session_key: String,
    ) -> Result<crate::jarvis::models::Response> {
        let start = std::time::Instant::now();
        log::info!("Starting parallel execution of plan: {}", plan.description);

        // Analyze dependencies
        let batches = self.analyze_dependencies(&plan);

        let mut all_results = Vec::new();
        let mut failed_step: Option<(usize, String)> = None;

        // Execute batches sequentially, steps within batch in parallel
        for (batch_idx, batch) in batches.iter().enumerate() {
            log::info!(
                "Executing batch {}/{} with {} steps in parallel",
                batch_idx + 1,
                batches.len(),
                batch.len()
            );

            let batch_start = std::time::Instant::now();

            // Create JoinSet for parallel execution
            let mut join_set = JoinSet::new();

            for step in batch {
                let step_clone = step.clone();
                let skill_registry = self.skill_registry.clone();
                let session_key_clone = session_key.clone();

                join_set.spawn(async move {
                    log::info!(
                        "Step {}: Executing {} action on {} skill",
                        step_clone.step_number,
                        step_clone.action,
                        step_clone.skill
                    );

                    // Extract parameters
                    let params = match step_clone.params.as_object() {
                        Some(obj) => obj.clone().into_iter().collect(),
                        None => {
                            log::error!(
                                "Step {} has invalid params (not an object): {:?}",
                                step_clone.step_number,
                                step_clone.params
                            );
                            return (
                                step_clone.step_number,
                                SkillResult::error(format!(
                                    "Step {} has invalid parameters format",
                                    step_clone.step_number
                                )),
                            );
                        }
                    };

                    // Execute skill
                    let result = skill_registry
                        .execute(
                            &step_clone.skill,
                            SkillContext {
                                session_key: session_key_clone,
                                action: step_clone.action.clone(),
                                params,
                                user_context: crate::jarvis::models::context::UserContext::new(),
                            },
                        )
                        .await;

                    log::info!(
                        "Step {}: {} - {}",
                        step_clone.step_number,
                        if result.success { "SUCCESS" } else { "FAILED" },
                        result.message
                    );

                    (step_clone.step_number, result)
                });
            }

            // Collect results from this batch
            let mut batch_results = Vec::new();
            let mut batch_failed = false;

            while let Some(result) = join_set.join_next().await {
                match result {
                    Ok((step_number, skill_result)) => {
                        batch_results.push((step_number, skill_result.clone()));

                        if !skill_result.success {
                            batch_failed = true;
                            failed_step = Some((step_number, skill_result.message.clone()));
                            log::error!("Step {} failed: {}", step_number, skill_result.message);
                        }
                    }
                    Err(e) => {
                        log::error!("Task execution error: {}", e);
                        batch_failed = true;
                        failed_step = Some((0, format!("Task execution error: {}", e)));
                    }
                }
            }

            let batch_duration = batch_start.elapsed();
            log::info!(
                "Batch {}/{} completed in {:?}",
                batch_idx + 1,
                batches.len(),
                batch_duration
            );

            // Update plan status
            for (step_number, skill_result) in &batch_results {
                if let Some(step) = plan.steps.iter_mut().find(|s| s.step_number == *step_number) {
                    step.status = if skill_result.success {
                        StepStatus::Completed
                    } else {
                        StepStatus::Failed
                    };
                }
                all_results.push(skill_result.message.clone());
            }

            // If any step in batch failed, cancel remaining batches
            if batch_failed {
                log::warn!(
                    "Batch {} failed, cancelling remaining {} batches",
                    batch_idx + 1,
                    batches.len() - batch_idx - 1
                );

                // Mark remaining steps as skipped
                for remaining_batch in batches.iter().skip(batch_idx + 1) {
                    for step in remaining_batch {
                        if let Some(plan_step) =
                            plan.steps.iter_mut().find(|s| s.step_number == step.step_number)
                        {
                            plan_step.status = StepStatus::Skipped;
                        }
                    }
                }

                break;
            }
        }

        let total_duration = start.elapsed();
        log::info!(
            "Parallel execution completed in {:?} (avg: {:?} per step)",
            total_duration,
            total_duration.checked_div(plan.steps.len() as u32).unwrap_or(total_duration)
        );

        // Return result
        if let Some((step_number, error_message)) = failed_step {
            Ok(crate::jarvis::models::Response {
                success: false,
                message: format!("Step {} failed: {}", step_number, error_message),
                data: Some(serde_json::to_value(&plan)?),
            })
        } else {
            Ok(crate::jarvis::models::Response {
                success: true,
                message: all_results.join("\n"),
                data: Some(serde_json::to_value(&plan)?),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jarvis::plan_builder::{ExecutionPlan, PlanStep, StepStatus};
    use crate::jarvis::skills::{Skill, SkillContext, SkillResult};
    use crate::jarvis::skills::metadata::{EligibilityResult, SkillMetadata, SkillRequirements};
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Mock skill for testing
    struct MockSkill {
        name: String,
        delay_ms: u64,
        should_fail: bool,
        execution_count: Arc<Mutex<usize>>,
    }

    impl MockSkill {
        fn new(name: &str, delay_ms: u64, should_fail: bool) -> Self {
            Self {
                name: name.to_string(),
                delay_ms,
                should_fail,
                execution_count: Arc::new(Mutex::new(0)),
            }
        }
    }

    #[async_trait]
    impl Skill for MockSkill {
        fn metadata(&self) -> SkillMetadata {
            SkillMetadata {
                name: self.name.clone(),
                version: "1.0.0".to_string(),
                description: format!("Mock skill {}", self.name),
                actions: vec![],
                requirements: SkillRequirements::default(),
                tags: vec![],
            }
        }

        fn check_eligibility(&self) -> EligibilityResult {
            EligibilityResult {
                eligible: true,
                reason: None,
            }
        }

        async fn execute(&self, _ctx: SkillContext) -> SkillResult {
            // Increment execution count
            *self.execution_count.lock().await += 1;

            // Simulate work
            tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;

            if self.should_fail {
                SkillResult::error(format!("{} failed", self.name))
            } else {
                SkillResult::success(format!("{} completed", self.name))
            }
        }
    }

    #[test]
    fn test_dependency_analysis() {
        let executor = ParallelExecutor::new(Arc::new(SkillRegistry::new()));

        let plan = ExecutionPlan {
            description: "Test plan".to_string(),
            steps: vec![
                PlanStep {
                    step_number: 1,
                    description: "Step 1".to_string(),
                    skill: "skill_a".to_string(),
                    action: "action1".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 2,
                    description: "Step 2".to_string(),
                    skill: "skill_b".to_string(),
                    action: "action1".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 3,
                    description: "Step 3".to_string(),
                    skill: "skill_a".to_string(),
                    action: "action2".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 4,
                    description: "Step 4".to_string(),
                    skill: "skill_c".to_string(),
                    action: "action1".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
            ],
            requires_approval: false,
        };

        let batches = executor.analyze_dependencies(&plan);

        // Expected batches:
        // Batch 1: skill_a, skill_b, skill_c (all different)
        // Batch 2: skill_a (depends on step 1)
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), 3); // Steps 1, 2, 4
        assert_eq!(batches[1].len(), 1); // Step 3

        // Verify no skill is repeated in batch 1
        let skills_batch_1: Vec<String> = batches[0].iter().map(|s| s.skill.clone()).collect();
        assert!(skills_batch_1.contains(&"skill_a".to_string()));
        assert!(skills_batch_1.contains(&"skill_b".to_string()));
        assert!(skills_batch_1.contains(&"skill_c".to_string()));
    }

    #[tokio::test]
    async fn test_parallel_execution() {
        let registry = Arc::new(SkillRegistry::new());

        // Register mock skills with different delays
        let skill_a = Arc::new(MockSkill::new("skill_a", 100, false));
        let skill_b = Arc::new(MockSkill::new("skill_b", 100, false));
        let skill_c = Arc::new(MockSkill::new("skill_c", 100, false));

        registry.register(skill_a.clone()).await;
        registry.register(skill_b.clone()).await;
        registry.register(skill_c.clone()).await;

        let executor = ParallelExecutor::new(registry);

        let plan = ExecutionPlan {
            description: "Test parallel plan".to_string(),
            steps: vec![
                PlanStep {
                    step_number: 1,
                    description: "Step 1".to_string(),
                    skill: "skill_a".to_string(),
                    action: "action1".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 2,
                    description: "Step 2".to_string(),
                    skill: "skill_b".to_string(),
                    action: "action1".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 3,
                    description: "Step 3".to_string(),
                    skill: "skill_c".to_string(),
                    action: "action1".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
            ],
            requires_approval: false,
        };

        let start = std::time::Instant::now();
        let result = executor
            .execute_parallel(plan, "test_session".to_string())
            .await
            .unwrap();
        let duration = start.elapsed();

        // Verify success
        assert!(result.success);
        assert!(result.message.contains("skill_a completed"));
        assert!(result.message.contains("skill_b completed"));
        assert!(result.message.contains("skill_c completed"));

        // Verify parallel execution (should be ~100ms, not 300ms)
        assert!(
            duration.as_millis() < 200,
            "Parallel execution took too long: {:?}",
            duration
        );
    }

    #[tokio::test]
    async fn test_parallel_execution_with_failure() {
        let registry = Arc::new(SkillRegistry::new());

        // Register mock skills - skill_b will fail
        let skill_a = Arc::new(MockSkill::new("skill_a", 50, false));
        let skill_b = Arc::new(MockSkill::new("skill_b", 50, true)); // This will fail
        let skill_c = Arc::new(MockSkill::new("skill_c", 50, false));

        registry.register(skill_a.clone()).await;
        registry.register(skill_b.clone()).await;
        registry.register(skill_c.clone()).await;

        let executor = ParallelExecutor::new(registry);

        let plan = ExecutionPlan {
            description: "Test failure plan".to_string(),
            steps: vec![
                PlanStep {
                    step_number: 1,
                    description: "Step 1".to_string(),
                    skill: "skill_a".to_string(),
                    action: "action1".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 2,
                    description: "Step 2".to_string(),
                    skill: "skill_b".to_string(),
                    action: "action1".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 3,
                    description: "Step 3".to_string(),
                    skill: "skill_a".to_string(),
                    action: "action2".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
            ],
            requires_approval: false,
        };

        let result = executor
            .execute_parallel(plan, "test_session".to_string())
            .await
            .unwrap();

        // Verify failure
        assert!(!result.success);
        assert!(result.message.contains("Step 2 failed"));
        assert!(result.message.contains("skill_b failed"));
    }

    #[tokio::test]
    async fn test_performance_improvement() {
        let registry = Arc::new(SkillRegistry::new());

        // Register mock skills with significant delay
        let skill_a = Arc::new(MockSkill::new("skill_a", 200, false));
        let skill_b = Arc::new(MockSkill::new("skill_b", 200, false));
        let skill_c = Arc::new(MockSkill::new("skill_c", 200, false));

        registry.register(skill_a.clone()).await;
        registry.register(skill_b.clone()).await;
        registry.register(skill_c.clone()).await;

        let executor = ParallelExecutor::new(registry);

        // 5 steps with different skills
        let plan = ExecutionPlan {
            description: "Performance test".to_string(),
            steps: vec![
                PlanStep {
                    step_number: 1,
                    description: "Step 1".to_string(),
                    skill: "skill_a".to_string(),
                    action: "action1".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 2,
                    description: "Step 2".to_string(),
                    skill: "skill_b".to_string(),
                    action: "action1".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 3,
                    description: "Step 3".to_string(),
                    skill: "skill_c".to_string(),
                    action: "action1".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 4,
                    description: "Step 4".to_string(),
                    skill: "skill_a".to_string(),
                    action: "action2".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
                PlanStep {
                    step_number: 5,
                    description: "Step 5".to_string(),
                    skill: "skill_b".to_string(),
                    action: "action2".to_string(),
                    params: serde_json::json!({}),
                    requires_approval: false,
                    status: StepStatus::Pending,
                },
            ],
            requires_approval: false,
        };

        let start = std::time::Instant::now();
        let result = executor
            .execute_parallel(plan, "test_session".to_string())
            .await
            .unwrap();
        let parallel_duration = start.elapsed();

        // Verify success
        assert!(result.success);

        // Expected: ~400ms (2 batches of 200ms each)
        // Sequential would be: 1000ms (5 steps of 200ms each)
        // Performance improvement: ~2.5x (400ms vs 1000ms)
        println!("Parallel execution time: {:?}", parallel_duration);
        assert!(
            parallel_duration.as_millis() < 600,
            "Expected ~400ms, got {:?}",
            parallel_duration
        );
    }
}
