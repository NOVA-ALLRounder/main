mod digest;
mod mock_workflow;
mod notion;

pub(crate) use digest::run_gmail_digest_pipeline;
pub(crate) use mock_workflow::ingest_mock_workflow_recommendation;

pub(crate) fn summarize_prompt(prompt: &str, max_chars: usize) -> String {
    let trimmed = prompt.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let short = trimmed.chars().take(max_chars).collect::<String>();
    format!("{}...", short)
}
