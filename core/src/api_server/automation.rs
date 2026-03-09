use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{ai_digest, db, llm_gateway, pattern_detector};

use super::{truncate_log_message, AppState};

#[derive(Deserialize)]
pub(crate) struct AiDigestRunRequest {
    pub(crate) text: Option<String>,
    pub(crate) scope_marker: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct AiDigestRunResponse {
    pub(crate) ok: bool,
    pub(crate) scope_marker: String,
    pub(crate) notion_url: Option<String>,
    pub(crate) webhook_url: String,
    pub(crate) status_code: u16,
    pub(crate) response_text: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct NewsSummaryRequest {
    pub(crate) title: Option<String>,
    pub(crate) link: Option<String>,
    pub(crate) source: Option<String>,
    #[serde(alias = "pubDate")]
    pub(crate) pub_date: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) scope_marker: Option<String>,
    pub(crate) telegram_chat_id: Option<String>,
    pub(crate) article_count: Option<usize>,
    pub(crate) topic: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct NewsSummaryResponse {
    pub(crate) ok: bool,
    pub(crate) summary: Value,
}

pub(crate) async fn run_ai_digest_handler(
    Json(req): Json<AiDigestRunRequest>,
) -> impl IntoResponse {
    let text = ai_digest::normalize_request_text(req.text.as_deref());
    match ai_digest::trigger_program_webhook(&text, req.scope_marker).await {
        Ok(result) => (
            StatusCode::OK,
            Json(AiDigestRunResponse {
                ok: true,
                scope_marker: result.scope_marker,
                notion_url: result.notion_url,
                webhook_url: result.webhook_url,
                status_code: result.status_code,
                response_text: result.response_text,
            }),
        )
            .into_response(),
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "ok": false,
                "error": err.to_string(),
            })),
        )
            .into_response(),
    }
}

pub(crate) async fn run_news_summary_handler(
    State(state): State<AppState>,
    Json(req): Json<NewsSummaryRequest>,
) -> impl IntoResponse {
    let Some(llm) = state.llm_client.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "ok": false,
                "error": "llm_client_not_available",
            })),
        )
            .into_response();
    };

    let clean = |v: Option<&str>, max_len: usize| -> String {
        v.unwrap_or("")
            .trim()
            .chars()
            .take(max_len)
            .collect::<String>()
    };

    let title = clean(req.title.as_deref(), 220);
    let link = clean(req.link.as_deref(), 500);
    let source = clean(req.source.as_deref(), 120);
    let pub_date = clean(req.pub_date.as_deref(), 120);
    let description = clean(req.description.as_deref(), 700);
    let scope_marker = clean(req.scope_marker.as_deref(), 120);
    let telegram_chat_id = clean(req.telegram_chat_id.as_deref(), 80);
    let topic = {
        let topic = clean(req.topic.as_deref(), 80);
        if topic.is_empty() {
            "뉴스".to_string()
        } else {
            topic
        }
    };
    let article_count = req.article_count.unwrap_or(5).clamp(1, 10);
    let transparency_note = "원문 접근 제한: RSS title/description 기반 요약";

    let prompt = format!(
        "당신은 뉴스 요약기입니다. 입력(title/link/source/pubDate/description)만 사용해 요약하세요.\n\
JSON 객체 하나만 출력하고 코드블록/마크다운/설명문을 추가하지 마세요.\n\
summary_bullets는 한국어 완결 문장 3~6개로 작성하세요.\n\
transparency_note에는 반드시 \"{}\"를 포함하세요.\n\n\
입력:\n\
- title: {}\n\
- link: {}\n\
- source: {}\n\
- pubDate: {}\n\
- description: {}\n\n\
출력 스키마:\n\
{{\n\
  \"title\": \"string\",\n\
  \"link\": \"string\",\n\
  \"source\": \"string\",\n\
  \"pubDate\": \"string\",\n\
  \"summary_bullets\": [\"string\", \"string\", \"string\"],\n\
  \"why_it_matters\": \"string\",\n\
  \"keywords\": [\"string\", \"string\", \"string\"],\n\
  \"transparency_note\": \"string\",\n\
  \"scope_marker\": \"string\",\n\
  \"telegram_chat_id\": \"string\",\n\
  \"article_count\": {}\n\
}}",
        transparency_note, title, link, source, pub_date, description, article_count
    );

    let messages = vec![
        json!({
            "role": "system",
            "content": "Return only one valid JSON object. No markdown."
        }),
        json!({
            "role": "user",
            "content": prompt
        }),
    ];

    let raw = match llm.chat_completion(messages).await {
        Ok(value) => value,
        Err(err) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": format!("llm_chat_completion_failed: {}", err),
                })),
            )
                .into_response();
        }
    };

    let parsed = match llm_gateway::recover_json(&raw) {
        Some(value) => value,
        None => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": "llm_output_not_json",
                    "raw_preview": truncate_log_message(&raw, 300),
                })),
            )
                .into_response();
        }
    };

    let get_str = |key: &str| -> Option<String> {
        parsed
            .get(key)
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };

    let mut bullets: Vec<String> = parsed
        .get("summary_bullets")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str())
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .take(6)
                .map(|value| value.chars().take(190).collect::<String>())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if bullets.len() < 3 {
        let fallback_title = if title.is_empty() {
            "주요 뉴스".to_string()
        } else {
            title.clone()
        };
        let fallback_desc = if description.is_empty() {
            "RSS 본문이 짧아 제목 중심으로 요약했습니다.".to_string()
        } else {
            description.chars().take(170).collect::<String>()
        };
        bullets = vec![
            format!("핵심: {}", fallback_title),
            format!("내용: {}", fallback_desc),
            format!(
                "영향: {} 맥락의 후속 의사결정에 참고할 가치가 있습니다.",
                topic
            ),
        ];
    }

    let mut keywords: Vec<String> = parsed
        .get("keywords")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str())
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .take(3)
                .map(|value| value.chars().take(30).collect::<String>())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    while keywords.len() < 3 {
        let filler = match keywords.len() {
            0 => topic.clone(),
            1 => "트렌드".to_string(),
            _ => "요약".to_string(),
        };
        keywords.push(filler);
    }

    let summary = json!({
        "title": get_str("title").unwrap_or_else(|| title.clone()),
        "link": get_str("link").unwrap_or_else(|| link.clone()),
        "source": get_str("source").unwrap_or_else(|| source.clone()),
        "pubDate": get_str("pubDate").unwrap_or_else(|| pub_date.clone()),
        "summary_bullets": bullets,
        "why_it_matters": get_str("why_it_matters").unwrap_or_else(|| format!("이 이슈는 {} 맥락의 실행 우선순위와 전략에 영향을 줄 수 있습니다.", topic)),
        "keywords": keywords,
        "transparency_note": transparency_note,
        "scope_marker": if scope_marker.is_empty() { get_str("scope_marker").unwrap_or_default() } else { scope_marker.clone() },
        "telegram_chat_id": if telegram_chat_id.is_empty() { get_str("telegram_chat_id").unwrap_or_default() } else { telegram_chat_id.clone() },
        "article_count": article_count,
    });

    (
        StatusCode::OK,
        Json(NewsSummaryResponse { ok: true, summary }),
    )
        .into_response()
}

pub(crate) async fn analyze_patterns() -> Json<Vec<String>> {
    Json(run_analysis_internal())
}

pub(crate) fn run_analysis_internal() -> Vec<String> {
    let detector = pattern_detector::PatternDetector::new();
    let patterns = detector.analyze();
    let preference_history = db::get_recent_recommendations(
        crate::recommendation_policy::auto_recommendation_history_limit(),
    )
    .unwrap_or_default();

    for pattern in &patterns {
        if !detector.should_recommend(pattern) {
            continue;
        }
        let proposal = crate::recommendation::AutomationProposal {
            title: format!("New Pattern: {}", pattern.description),
            summary: format!(
                "Detected {} repeats across {} distinct day(s). AI suggests automating this.",
                pattern.occurrences, pattern.distinct_days
            ),
            trigger: format!("Pattern Type: {:?}", pattern.pattern_type),
            actions: vec!["Analyze".to_string(), "Automate".to_string()],
            n8n_prompt: format!("Create an automation for: {}", pattern.description),
            confidence: pattern.similarity_score,
            evidence: vec![
                format!("Pattern: {}", pattern.description),
                format!("Frequency: {} occurrences", pattern.occurrences),
                format!("Span: {} distinct day(s)", pattern.distinct_days),
                format!(
                    "Work context: weekday {} / work-hour {} occurrences",
                    pattern.weekday_occurrences, pattern.work_hour_occurrences
                ),
            ],
            pattern_id: Some(pattern.pattern_id.clone()),
            category: crate::recommendation_policy::CATEGORY_UNKNOWN.to_string(),
            business_score: 0.0,
        };
        let mut proposal = proposal;
        let decision = crate::recommendation_policy::apply_mvp_policy(&mut proposal, Some(pattern));
        if !decision.accepted {
            continue;
        }
        crate::recommendation_policy::apply_recommendation_preferences(
            &mut proposal,
            &preference_history,
        );
        if !auto_recommendation_allowed(&proposal, "api.patterns.analyze") {
            continue;
        }
        if let Err(err) = db::insert_recommendation(&proposal) {
            eprintln!("Failed to save pattern: {}", err);
        }
    }

    patterns
        .into_iter()
        .filter(|pattern| detector.should_recommend(pattern))
        .map(|pattern| {
            format!(
                "{} ({} occurrences)",
                pattern.description, pattern.occurrences
            )
        })
        .collect()
}

fn auto_recommendation_allowed(
    proposal: &crate::recommendation::AutomationProposal,
    source: &str,
) -> bool {
    let history_limit = crate::recommendation_policy::auto_recommendation_history_limit();
    let recent = db::get_recent_recommendations(history_limit).unwrap_or_default();
    let decision = crate::recommendation_policy::admit_auto_recommendation(proposal, &recent);
    if decision.accepted {
        return true;
    }

    eprintln!(
        "🧹 [{}] Suppressing auto recommendation: {} [{} / {} / {:.2}] {}",
        source,
        proposal.title,
        decision.pending_same_category,
        decision.pending_limit,
        decision.priority_score,
        decision.reasons.join(", ")
    );
    false
}
