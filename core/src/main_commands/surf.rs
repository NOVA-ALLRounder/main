use local_os_agent::llm_gateway;
use std::sync::Arc;
use tracing::{error, info, warn};

pub(crate) async fn handle_surf_repl_command(
    parts: &[&str],
    llm_client: Option<&Arc<dyn llm_gateway::LLMClient>>,
) {
    if parts.len() < 2 {
        warn!("Usage: surf <goal>");
        return;
    }
    let goal = parts[1..].join(" ");

    if let Some(brain) = llm_client {
        let planner = local_os_agent::controller::planner::Planner::new(brain.clone(), None);
        match planner.run_goal_tracked(&goal, None).await {
            Ok(outcome) => info!(
                "✅ Surf completed (run_id={}, planner={}, execution={}, business={})",
                outcome.run_id,
                outcome.planner_complete,
                outcome.execution_complete,
                outcome.business_complete
            ),
            Err(e) => error!("❌ Surf failed: {}", e),
        }
    } else {
        warn!("⚠️  LLM Client not available.");
    }
}
