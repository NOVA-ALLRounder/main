use crate::launch_eval::*;

pub(super) fn launch_event_candidate(
    event: &crate::db::LaunchOpsEventRecord,
) -> Option<LaunchEvalCandidate> {
    if event.outcome != "success" {
        return None;
    }
    if is_synthetic_launch_event(event) {
        return None;
    }
    let command = event.command.as_deref()?.trim();
    if !matches!(
        command,
        "calendar_today" | "calendar_week" | "gmail_list" | "build_workflow" | "ai_digest_program"
    ) {
        return None;
    }
    if event.message_preview.trim().is_empty() {
        return None;
    }

    let scope = candidate_scope(event.memory_scope.as_deref().unwrap_or("global"), command);
    let ai_digest_mock = if command == "ai_digest_program" {
        Some(AiDigestMock {
            status: "ok".to_string(),
            notion_url: Some(format!(
                "https://www.notion.so/launch-eval-{}",
                slug_id(&event.message_preview)
            )),
            top_headlines_text: Some(
                "1. Launch candidate headline A\n2. Launch candidate headline B".to_string(),
            ),
        })
    } else {
        None
    };

    let expect = if let Some(mock) = ai_digest_mock.as_ref() {
        LaunchEvalChatExpectation {
            command: Some(command.to_string()),
            response_contains: vec![
                mock.notion_url.clone().unwrap_or_default(),
                "Launch candidate headline A".to_string(),
            ],
            response_contains_any: vec![],
            response_not_contains: vec![],
        }
    } else {
        LaunchEvalChatExpectation {
            command: Some(command.to_string()),
            response_contains: vec![],
            response_contains_any: vec![],
            response_not_contains: vec![],
        }
    };

    let scenario = LaunchEvalScenario::Chat {
        id: candidate_id("chat", command, &event.message_preview),
        description: Some(format!(
            "Generated from launch ops route '{}' (channel={}).",
            event.route_kind,
            event.channel.as_deref().unwrap_or("unknown")
        )),
        request: LaunchEvalChatInput {
            message: event.message_preview.clone(),
            channel: scope.channel.clone(),
            chat_type: scope.chat_type.clone(),
            sender: scope.sender.clone(),
            mentioned: Some(false),
        },
        ai_digest_mock,
        expect,
    };

    Some(LaunchEvalCandidate {
        id: scenario_id(&scenario),
        provenance: "real".to_string(),
        source_kind: "launch_ops".to_string(),
        scenario_kind: "chat".to_string(),
        title: format!("Chat route: {}", summarize_message(&event.message_preview)),
        score: launch_event_score(event),
        command: Some(command.to_string()),
        request_message: event.message_preview.clone(),
        rationale: vec![
            format!("route_kind={}", event.route_kind),
            format!(
                "memory_hits=intent:{} request:{} execution:{}",
                event.intent_memory_hit, event.request_memory_hit, event.execution_memory_hit
            ),
            format!("confidence={:.2}", event.confidence.unwrap_or(0.0)),
        ],
        yaml: render_scenario_yaml(&scenario),
    })
}

fn is_synthetic_launch_event(event: &crate::db::LaunchOpsEventRecord) -> bool {
    event
        .channel
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("dogfood"))
        .unwrap_or(false)
        || event
            .note
            .as_deref()
            .map(|value| {
                let normalized = value.to_ascii_lowercase();
                normalized.contains("synthetic dogfood")
                    || normalized.contains("launch.eval")
                    || normalized.contains("launch.dogfood.synthetic")
            })
            .unwrap_or(false)
}
