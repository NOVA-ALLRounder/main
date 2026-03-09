use crate::{integrations, workflow_intake};

use super::{chat_support::parse_u32_param, run_analysis_internal};

pub(crate) async fn execute_chat_command(
    command: &str,
    intent: &serde_json::Value,
    message: &str,
) -> String {
    match command {
        "analyze_patterns" => {
            let results = run_analysis_internal();
            if results.is_empty() {
                "🔍 분석 완료! 새로운 패턴을 찾지 못했습니다.".to_string()
            } else {
                format!("🔍 분석 완료!:\n{}", results.join("\n"))
            }
        }
        "gmail_list" => {
            let count = parse_u32_param(intent["params"].get("count"), 5, 1, 20);
            match integrations::gmail::GmailClient::new().await {
                Ok(client) => match client.list_messages(count).await {
                    Ok(msgs) => {
                        if msgs.is_empty() {
                            "📭 새 메일이 없습니다.".to_string()
                        } else {
                            let mut s = format!("📧 최근 이메일 {}건:\n", count);
                            for (_, subj, from) in msgs {
                                s.push_str(&format!("• {} ({})\n", subj, from));
                            }
                            s
                        }
                    }
                    Err(e) => format!("❌ 이메일 가져오기 실패: {}", e),
                },
                Err(e) => format!("⚠️ Gmail 인증 실패: {}", e),
            }
        }
        "calendar_today" => match integrations::calendar::CalendarClient::new().await {
            Ok(client) => match client.list_today().await {
                Ok(events) => {
                    if events.is_empty() {
                        "📅 오늘 일정이 없습니다.".to_string()
                    } else {
                        let mut s = String::from("📅 오늘 일정:\n");
                        for (_, summary, start_time) in events {
                            s.push_str(&format!("• {} ({})\n", summary, start_time));
                        }
                        s
                    }
                }
                Err(e) => format!("❌ 일정 확인 실패: {}", e),
            },
            Err(e) => format!("⚠️ Calendar 인증 실패: {}", e),
        },
        "calendar_week" => match integrations::calendar::CalendarClient::new().await {
            Ok(client) => match client.list_week().await {
                Ok(events) => {
                    if events.is_empty() {
                        "📅 이번 주 일정이 없습니다.".to_string()
                    } else {
                        let mut s = String::from("📅 이번 주 일정:\n");
                        for (_, summary, start_time) in events {
                            s.push_str(&format!("• {} ({})\n", summary, start_time));
                        }
                        s
                    }
                }
                Err(e) => format!("❌ 일정 확인 실패: {}", e),
            },
            Err(e) => format!("⚠️ Calendar 인증 실패: {}", e),
        },
        "system_status" => {
            let mut rm = crate::monitor::ResourceMonitor::new();
            format!("📊 {}", rm.get_status())
        }
        "build_workflow" => {
            let prompt_str = intent["params"]["prompt"]
                .as_str()
                .or_else(|| intent["params"]["description"].as_str())
                .unwrap_or(message);
            match workflow_intake::queue_manual_workflow_recommendation(
                prompt_str,
                "api.chat.build_workflow",
            ) {
                Ok(outcome) => {
                    let rec_id = outcome.recommendation_id;
                    let inserted = outcome.inserted;
                    let queued = if inserted { "생성" } else { "재사용" };
                    format!(
                        "📝 워크플로우 제안을 {}했습니다 (ID: {}).\n승인 게이트 정책상 즉시 생성은 차단되며, `/api/recommendations/{}/approve` 또는 CLI `approve {}`로 승인 후 생성됩니다.",
                        queued, rec_id, rec_id, rec_id
                    )
                }
                Err(e) => format!("❌ 워크플로우 제안 저장 실패: {}", e),
            }
        }
        "create_routine" => {
            let params = intent["params"].as_object();
            if let Some(p) = params {
                let name = p
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("New Routine");
                let cron = p
                    .get("cron")
                    .and_then(|v| v.as_str())
                    .unwrap_or("* * * * *");

                if std::str::FromStr::from_str(cron as &str)
                    .map(|_: cron::Schedule| ())
                    .is_err()
                {
                    format!("❌ 잘못된 Cron 표현식입니다: {}", cron)
                } else {
                    let prompt = p
                        .get("prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Check status");

                    match crate::db::create_routine(name, cron, prompt) {
                        Ok(_id) => format!(
                            "✅ 루틴이 등록되었습니다!\n• 이름: {}\n• 주기: {}\n• 명령: {}",
                            name, cron, prompt
                        ),
                        Err(e) => format!("❌ 루틴 등록 실패: {}", e),
                    }
                }
            } else {
                "❌ 루틴 정보를 파악할 수 없습니다.".to_string()
            }
        }
        "help" => {
            "💡 사용 가능한 명령:\n• '이메일 보여줘'\n• '오늘 일정 뭐야?'\n• '매일 아침 9시 뉴스 요약해줘' (New!)"
                .to_string()
        }
        _ => "🤔 요청을 정확히 해석하지 못했어요.\n원하는 작업을 한 문장으로 더 구체적으로 말해줘.\n예: `오늘 일정 보여줘`, `최근 메일 5개 요약해줘`, `n8n 열어줘`".to_string(),
    }
}
