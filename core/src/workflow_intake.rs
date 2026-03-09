mod collector;
mod manual;
mod support;

pub use collector::{ingest_latest_collector_handoff, CollectorHandoffIngestOutcome};
pub use manual::{
    insert_or_get_recommendation_id, queue_manual_workflow_recommendation,
    ManualWorkflowQueueOutcome,
};

#[cfg(test)]
#[path = "workflow_intake/tests.rs"]
mod tests;
