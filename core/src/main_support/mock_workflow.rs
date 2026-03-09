use anyhow::anyhow;
use local_os_agent::{llm_gateway, recommendation, recommendation_executor, workflow_intake};
use std::sync::Arc;

fn load_mock_workflow_proposal() -> anyhow::Result<recommendation::AutomationProposal> {
    let path = std::env::var("STEER_WORKFLOW_MOCK_FILE")
        .unwrap_or_else(|_| "core/mock/workflow_received_mock.json".to_string());
    let raw = std::fs::read_to_string(&path)?;
    let proposal = serde_json::from_str::<recommendation::AutomationProposal>(&raw)?;
    if proposal.n8n_prompt.trim().is_empty() {
        return Err(anyhow!("mock workflow has empty n8n_prompt: {}", path));
    }
    Ok(proposal)
}

pub(crate) async fn ingest_mock_workflow_recommendation(
    llm_client: Option<Arc<dyn llm_gateway::LLMClient>>,
) -> anyhow::Result<()> {
    let proposal = load_mock_workflow_proposal()?;
    let (rec_id, inserted) = workflow_intake::insert_or_get_recommendation_id(&proposal)?;

    if inserted {
        println!(
            "📥 Mock workflow ingested as pending recommendation [{}] {}",
            rec_id, proposal.title
        );
    } else {
        println!(
            "📥 Mock workflow already exists; reusing recommendation [{}] {}",
            rec_id, proposal.title
        );
    }

    if local_os_agent::env_flag("STEER_TEST_ASSUME_APPROVED") {
        recommendation_executor::maybe_assume_approved_for_test(rec_id)?;
        match recommendation_executor::approve_and_execute_recommendation(rec_id, llm_client).await
        {
            Ok(outcome) => println!(
                "✅ [TEST] approve-assumed pipeline completed. Workflow ID: {} (reused={})",
                outcome.workflow_id, outcome.reused_existing
            ),
            Err(e) => println!("❌ [TEST] approve-assumed pipeline failed: {}", e),
        }
    }

    Ok(())
}
