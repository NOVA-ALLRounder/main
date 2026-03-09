use crate::{db, recommendation::AutomationProposal};
use anyhow::{anyhow, Result};

use super::support::{find_recommendation_id_by_fingerprint, summarize_prompt};

#[derive(Debug, Clone)]
pub struct ManualWorkflowQueueOutcome {
    pub recommendation_id: i64,
    pub inserted: bool,
}

pub fn insert_or_get_recommendation_id(proposal: &AutomationProposal) -> Result<(i64, bool)> {
    let fp = proposal.fingerprint();
    let inserted = db::insert_recommendation(proposal)?;
    let rec_id = find_recommendation_id_by_fingerprint(&fp)?
        .ok_or_else(|| anyhow!("failed to resolve recommendation id after insert"))?;
    Ok((rec_id, inserted))
}

pub fn queue_manual_workflow_recommendation(
    prompt: &str,
    source: &str,
) -> Result<ManualWorkflowQueueOutcome> {
    let prompt_trimmed = prompt.trim();
    if prompt_trimmed.is_empty() {
        return Err(anyhow!("workflow prompt is empty"));
    }
    let short = summarize_prompt(prompt_trimmed, 48);
    let mut proposal = AutomationProposal {
        title: format!("Manual Workflow: {}", short),
        summary: format!(
            "Manual workflow request captured from {} (approval required before creation).",
            source
        ),
        trigger: "Manual workflow request".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.6,
        n8n_prompt: prompt_trimmed.to_string(),
        evidence: vec![
            format!("source={}", source),
            format!("prompt={}", summarize_prompt(prompt_trimmed, 160)),
        ],
        pattern_id: None,
        category: crate::recommendation_policy::CATEGORY_UNKNOWN.to_string(),
        business_score: 0.0,
    };
    let preference_history = db::get_recent_recommendations(
        crate::recommendation_policy::auto_recommendation_history_limit(),
    )
    .unwrap_or_default();
    crate::recommendation_policy::apply_recommendation_preferences(
        &mut proposal,
        &preference_history,
    );

    let (recommendation_id, inserted) = insert_or_get_recommendation_id(&proposal)?;
    Ok(ManualWorkflowQueueOutcome {
        recommendation_id,
        inserted,
    })
}
