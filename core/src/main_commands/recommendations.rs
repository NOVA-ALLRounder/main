use local_os_agent::{
    db, feedback_collector, llm_gateway, monitor, pattern_detector, recommendation_executor,
    workflow_intake,
};
use std::sync::Arc;

use crate::main_support::ingest_mock_workflow_recommendation;

pub(crate) fn handle_status_command(res_mon: &mut monitor::ResourceMonitor) {
    println!("📊 System Status:");
    println!("   {}", res_mon.get_status());
    println!("   Top Apps:");
    for (name, usage) in res_mon.get_high_usage_apps() {
        println!("   - {}: {:.1}%", name, usage);
    }
}

pub(crate) fn handle_recommendations_command(parts: &[&str]) {
    let limit = parts
        .get(1)
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(5);
    match db::list_recommendations("pending", limit) {
        Ok(recs) => {
            if recs.is_empty() {
                println!("(No pending recommendations)");
            } else {
                println!("🧩 Pending recommendations:");
                for rec in recs {
                    println!(
                        "  [{}] {} (confidence {:.2})",
                        rec.id, rec.title, rec.confidence
                    );
                    println!("       Trigger: {}", rec.trigger);
                    println!("       Summary: {}", rec.summary);
                }
            }
        }
        Err(e) => println!("❌ Failed to load recommendations: {}", e),
    }
}

pub(crate) async fn handle_approve_command(
    parts: &[&str],
    llm_client: Option<Arc<dyn llm_gateway::LLMClient>>,
) {
    if parts.len() < 2 {
        println!("Usage: approve <id>");
        return;
    }
    let id: i64 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => {
            println!("Usage: approve <id>");
            return;
        }
    };
    println!("🏗️  Running approval pipeline for recommendation {}...", id);
    match recommendation_executor::approve_and_execute_recommendation(id, llm_client).await {
        Ok(outcome) => {
            if outcome.reused_existing {
                println!(
                    "♻️  Workflow reused (already provisioned). ID: {}",
                    outcome.workflow_id
                );
            } else {
                println!("✅ Workflow created! ID: {}", outcome.workflow_id);
            }
            if outcome.approved_now {
                println!("📝 Recommendation {} marked as approved.", id);
            } else {
                println!("ℹ️ Recommendation {} was already approved.", id);
            }
        }
        Err(e) => println!("❌ Approval pipeline failed: {}", e),
    }
}

pub(crate) async fn handle_approve_test_command(
    parts: &[&str],
    llm_client: Option<Arc<dyn llm_gateway::LLMClient>>,
) {
    if parts.len() < 2 {
        println!("Usage: approve_test <id>");
        return;
    }
    let id: i64 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => {
            println!("Usage: approve_test <id>");
            return;
        }
    };

    if let Err(e) = recommendation_executor::maybe_assume_approved_for_test(id) {
        println!("❌ approve_test precheck failed: {}", e);
        return;
    }
    match recommendation_executor::approve_and_execute_recommendation(id, llm_client).await {
        Ok(outcome) => {
            if outcome.reused_existing {
                println!(
                    "♻️  [TEST] Workflow reused (already provisioned). ID: {}",
                    outcome.workflow_id
                );
            } else {
                println!("✅ [TEST] Workflow created! ID: {}", outcome.workflow_id);
            }
        }
        Err(e) => println!("❌ [TEST] Approval pipeline failed: {}", e),
    }
}

pub(crate) async fn handle_ingest_mock_workflow_command(
    llm_client: Option<Arc<dyn llm_gateway::LLMClient>>,
) {
    if let Err(e) = ingest_mock_workflow_recommendation(llm_client).await {
        println!("❌ Mock workflow ingest failed: {}", e);
    }
}

pub(crate) fn handle_ingest_handoff_command(parts: &[&str]) {
    let config_override = parts.get(1).copied();
    match workflow_intake::ingest_latest_collector_handoff(config_override) {
        Ok(outcome) => {
            println!(
                "📥 Handoff ingest status={} detail={}",
                outcome.status, outcome.detail
            );
            if let Some(pkg) = outcome.package_id {
                println!("   package_id={}", pkg);
            }
            if let Some(id) = outcome.recommendation_id {
                println!("   recommendation_id={} inserted={}", id, outcome.inserted);
            }
        }
        Err(e) => println!("❌ Handoff ingest failed: {}", e),
    }
}

pub(crate) fn handle_reject_command(parts: &[&str]) {
    if parts.len() < 2 {
        println!("Usage: reject <id>");
        return;
    }
    let id: i64 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => {
            println!("Usage: reject <id>");
            return;
        }
    };
    match db::update_recommendation_review_status(id, "rejected") {
        Ok(()) => println!("🗑️  Recommendation {} rejected.", id),
        Err(e) => println!("❌ Failed to reject recommendation: {}", e),
    }
}

pub(crate) fn handle_build_workflow_command(parts: &[&str]) {
    if parts.len() < 2 {
        println!("Usage: build_workflow <prompt>");
        return;
    }
    let prompt = parts[1..].join(" ");
    match workflow_intake::queue_manual_workflow_recommendation(&prompt, "cli.build_workflow") {
        Ok(outcome) => {
            let rec_id = outcome.recommendation_id;
            let inserted = outcome.inserted;
            if inserted {
                println!("📝 Recommendation queued [{}] as pending approval.", rec_id);
            } else {
                println!(
                    "📝 Existing recommendation reused [{}] as pending/approved history.",
                    rec_id
                );
            }
            println!(
                "   Approval gate enforced: run `approve {}` to create in n8n.",
                rec_id
            );
            println!("   Rejection path: run `reject {}`.", rec_id);
        }
        Err(e) => println!("❌ Failed to queue workflow recommendation: {}", e),
    }
}

pub(crate) async fn handle_analyze_patterns_command(
    llm_client: Option<&Arc<dyn llm_gateway::LLMClient>>,
) {
    println!("🔍 Analyzing behavior patterns...");
    let detector = pattern_detector::PatternDetector::new();
    let patterns = detector.analyze();

    if patterns.is_empty() {
        println!("   (No significant patterns detected yet)");
        println!("   Keep using your computer - patterns will be detected over time.");
        return;
    }

    println!("   Found {} patterns:", patterns.len());
    for pattern in &patterns {
        println!(
            "   📊 {} ({} occurrences, {:.0}% similarity)",
            pattern.description,
            pattern.occurrences,
            pattern.similarity_score * 100.0
        );
    }

    if let Some(brain) = llm_client {
        println!("\n🤖 Generating workflow recommendations...");
        let preference_history = db::get_recent_recommendations(
            local_os_agent::recommendation_policy::auto_recommendation_history_limit(),
        )
        .unwrap_or_default();
        for pattern in patterns {
            if !detector.should_recommend(&pattern) {
                continue;
            }
            match brain
                .generate_recommendation_from_pattern(&pattern.description, &pattern.sample_events)
                .await
            {
                Ok(mut proposal) => {
                    proposal
                        .evidence
                        .push(format!("Pattern: {}", pattern.description));
                    proposal.evidence.push(format!(
                        "Frequency: {} occurrences in last 7 days",
                        pattern.occurrences
                    ));
                    proposal
                        .evidence
                        .push(format!("Span: {} distinct day(s)", pattern.distinct_days));
                    let decision = local_os_agent::recommendation_policy::apply_mvp_policy(
                        &mut proposal,
                        Some(&pattern),
                    );
                    if !decision.accepted {
                        println!(
                            "   🧹 Skipped non-work recommendation: {} [{} {:.2}]",
                            proposal.title, decision.category, decision.business_score
                        );
                        continue;
                    }
                    local_os_agent::recommendation_policy::apply_recommendation_preferences(
                        &mut proposal,
                        &preference_history,
                    );

                    if proposal.confidence >= 0.7 {
                        let history_limit =
                            local_os_agent::recommendation_policy::auto_recommendation_history_limit();
                        let pending =
                            db::get_recent_recommendations(history_limit).unwrap_or_default();
                        let queue_decision =
                            local_os_agent::recommendation_policy::admit_auto_recommendation(
                                &proposal, &pending,
                            );
                        if !queue_decision.accepted {
                            println!(
                                "   🧹 Suppressed auto recommendation: {} [{} / {} / {:.2}] {}",
                                proposal.title,
                                queue_decision.pending_same_category,
                                queue_decision.pending_limit,
                                queue_decision.priority_score,
                                queue_decision.reasons.join(", ")
                            );
                            continue;
                        }
                        if let Ok(true) = db::insert_recommendation(&proposal) {
                            println!(
                                "   ✨ New recommendation: {} (confidence: {:.0}%)",
                                proposal.title,
                                proposal.confidence * 100.0
                            );
                        }
                    }
                }
                Err(e) => println!("   ⚠️  Skipped pattern: {}", e),
            }
        }
        println!("\nRun 'recommendations' to see pending recommendations.");
    }
}

pub(crate) fn handle_quality_command() {
    let collector = feedback_collector::FeedbackCollector::new();
    let metrics = collector.get_quality_metrics();
    println!("📈 Workflow Quality Metrics:");
    println!("   {}", metrics);
}
