use super::*;

pub(super) struct PlannerRunSetup {
    pub(super) scenario_mode: bool,
    pub(super) deterministic_goal_mode: bool,
    pub(super) allow_deterministic_fallback: bool,
    pub(super) allow_review_loop_override: bool,
    pub(super) node_capture_enabled: bool,
    pub(super) node_capture_all: bool,
    pub(super) node_capture_dir: Option<PathBuf>,
    pub(super) session: Session,
    pub(super) history: Vec<String>,
    pub(super) max_wall_duration: Duration,
    pub(super) max_repeat_loop_hits: usize,
    pub(super) max_attempts_per_plan_key: usize,
}

impl Planner {
    pub(super) async fn prepare_goal_run(
        &self,
        goal: &str,
        session_key: Option<&str>,
    ) -> Result<PlannerRunSetup> {
        let scenario_mode = Self::scenario_mode_enabled();
        let deterministic_goal_mode =
            !scenario_mode && Self::should_use_deterministic_goal_autoplan(goal);
        if deterministic_goal_mode {
            println!("🧭 Deterministic goal autoplan enabled (script-like goal detected).");
        }

        let test_context = Self::env_truthy("STEER_TEST_MODE") || Self::env_truthy("CI");
        let deterministic_fallback_requested =
            Self::env_truthy("STEER_ALLOW_DETERMINISTIC_FALLBACK");
        let review_loop_override_requested = Self::env_truthy("STEER_ALLOW_REVIEW_LOOP_OVERRIDE");
        if deterministic_fallback_requested && !test_context {
            return Err(anyhow::anyhow!(
                "STEER_ALLOW_DETERMINISTIC_FALLBACK is test-only (requires STEER_TEST_MODE=1 or CI=1)."
            ));
        }
        if review_loop_override_requested && !test_context {
            return Err(anyhow::anyhow!(
                "STEER_ALLOW_REVIEW_LOOP_OVERRIDE is test-only (requires STEER_TEST_MODE=1 or CI=1)."
            ));
        }

        let allow_deterministic_fallback = deterministic_fallback_requested && test_context;
        let allow_review_loop_override =
            review_loop_override_requested && allow_deterministic_fallback;
        let require_primary_planner =
            Self::env_truthy_default("STEER_REQUIRE_PRIMARY_PLANNER", true);
        let allow_scenario_mode = Self::env_truthy("STEER_ALLOW_SCENARIO_MODE");
        if scenario_mode && require_primary_planner && !allow_scenario_mode {
            return Err(anyhow::anyhow!(
                "Scenario mode fallback is disabled by policy (set STEER_ALLOW_SCENARIO_MODE=1 only for explicit test runs)."
            ));
        }

        let (node_capture_enabled, node_capture_all, node_capture_dir) =
            Self::prepare_node_capture()?;
        let (session, mut history) = Self::prepare_session(goal, session_key)?;
        Self::run_standard_cleanup_preset(goal, &mut history).await;

        let max_wall_seconds = Self::env_u64("STEER_GOAL_MAX_WALL_SEC", 240);
        let max_wall_duration = Duration::from_secs(max_wall_seconds.max(30));
        let max_repeat_loop_hits = Self::env_usize("STEER_MAX_REPEAT_LOOP_HITS", 6).max(2);
        let max_attempts_per_plan_key =
            Self::env_usize("STEER_MAX_ATTEMPTS_PER_PLAN_KEY", 12).max(3);

        Ok(PlannerRunSetup {
            scenario_mode,
            deterministic_goal_mode,
            allow_deterministic_fallback,
            allow_review_loop_override,
            node_capture_enabled,
            node_capture_all,
            node_capture_dir,
            session,
            history,
            max_wall_duration,
            max_repeat_loop_hits,
            max_attempts_per_plan_key,
        })
    }

    fn prepare_node_capture() -> Result<(bool, bool, Option<PathBuf>)> {
        let mut node_capture_enabled = Self::env_truthy("STEER_NODE_CAPTURE");
        let node_capture_all = Self::env_truthy("STEER_NODE_CAPTURE_ALL");
        let mut node_capture_dir: Option<PathBuf> = None;

        if node_capture_enabled {
            let dir = std::env::var("STEER_NODE_CAPTURE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    PathBuf::from(format!(
                        "scenario_results/node_evidence_{}",
                        Utc::now().format("%Y%m%d_%H%M%S")
                    ))
                });

            match std::fs::create_dir_all(&dir) {
                Ok(_) => {
                    println!("📸 Node capture enabled: {}", dir.display());
                    node_capture_dir = Some(dir);
                }
                Err(e) => {
                    println!(
                        "⚠️ Node capture disabled: failed to create dir '{}': {}",
                        dir.display(),
                        e
                    );
                    node_capture_enabled = false;
                }
            }
        }

        Ok((node_capture_enabled, node_capture_all, node_capture_dir))
    }

    fn prepare_session(goal: &str, session_key: Option<&str>) -> Result<(Session, Vec<String>)> {
        let _ = crate::session_store::init_session_store();
        let mut session = Session::new(goal, session_key);
        session.add_message("user", goal);

        let mut history = Vec::new();
        if let Err(e) = heuristics::preflight_permissions() {
            println!("❌ Preflight failed: {}", e);
            history.push(format!("PREFLIGHT_PERMISSIONS_FAILED: {}", e));
            return Err(e);
        }
        history.push("PREFLIGHT_PERMISSIONS_OK".to_string());

        if let Err(e) = heuristics::verify_screen_capture() {
            history.push(format!("PREFLIGHT_SCREEN_CAPTURE_FAILED: {}", e));
            return Err(e);
        }
        history.push("PREFLIGHT_SCREEN_CAPTURE_OK".to_string());

        Ok((session, history))
    }
}
