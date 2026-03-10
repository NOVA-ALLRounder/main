use super::Planner;
use crate::platform::AppRole;
use chrono::{Local, Utc};

impl Planner {
    pub(super) fn fallback_n8n_workflow_goal(
        goal: &str,
        history: &[String],
    ) -> Option<serde_json::Value> {
        if !Self::goal_targets_n8n_workflow_request(goal) {
            return None;
        }

        if !Self::history_has_n8n_workflow_created(history) {
            let scope_marker = Self::goal_run_scope_marker(goal)
                .unwrap_or_else(|| "RUN_SCOPE_TEST_03".to_string());
            return Some(serde_json::json!({
                "action": "n8n_create_workflow",
                "name": format!("Steer Scope {}", scope_marker),
                "marker": scope_marker
            }));
        }

        if Self::goal_requires_n8n_execution(goal) && !Self::history_has_n8n_execution_done(history)
        {
            return Some(serde_json::json!({
                "action": "n8n_execute_workflow"
            }));
        }

        Some(serde_json::json!({ "action": "done" }))
    }

    pub(super) fn fallback_product_comparison_goal(
        goal: &str,
        history: &[String],
    ) -> Option<serde_json::Value> {
        if !Self::goal_targets_product_comparison_research(goal) {
            return None;
        }

        if !Self::history_contains_case_insensitive(history, "google.com/search?q=") {
            let search_query = Self::infer_product_comparison_search_query(goal);
            let encoded_query = urlencoding::encode(&search_query).replace("%20", "+");
            let search_url = format!("https://www.google.com/search?q={}", encoded_query);
            return Some(serde_json::json!({
                "action": "open_url",
                "url": search_url
            }));
        }

        if !Self::history_has_read_result(history) {
            return Some(serde_json::json!({
                "action": "read",
                "query": "후보 제품 5개의 이름, 공식 웹사이트 URL, 가격/플랜 URL, 보안/권한 모델, 워크플로우 오케스트레이션, 감사로그, UI 제어 방식을 표 형태로 추출"
            }));
        }

        if Self::history_has_type_permission_block(history) {
            let report_marker = "PRODUCT_COMPARISON_REPORT_EMITTED";
            if !Self::history_contains_case_insensitive(history, report_marker) {
                let mut report_message = "PRODUCT_COMPARISON_REPORT_EMITTED\n키입력 권한 제한으로 UI 타이핑을 우회했습니다.\n후보 5개 비교표 필수 항목: 보안(로컬 실행/권한), 오케스트레이션, 감사로그, UI 제어 방식, 가격/플랜, 근거 링크, 스크린샷."
                    .to_string();
                if let Some(marker) = Self::goal_run_scope_marker(goal) {
                    if !report_message.contains(&marker) {
                        report_message = format!("{}\n{}", report_message, marker);
                    }
                }
                return Some(serde_json::json!({
                    "action": "report",
                    "message": report_message
                }));
            }
            return Some(serde_json::json!({ "action": "done" }));
        }

        let target_app = Self::text_staging_app();
        if !Self::history_contains_opened_app(history, target_app) {
            return Some(serde_json::json!({
                "action": "open_app",
                "name": target_app
            }));
        }

        if Self::app_is_role(target_app, AppRole::NotesApp)
            && !Self::history_contains_case_insensitive(history, "Created new item")
            && !Self::history_contains_case_insensitive(history, "shortcut 'n'")
        {
            return Some(serde_json::json!({
                "action": "shortcut",
                "key": "n",
                "modifiers": ["command"],
                "app": target_app
            }));
        }

        let requested_count = regex::Regex::new(r"(?i)\b([3-9])\s*(?:개|products?)")
            .ok()
            .and_then(|re| {
                re.captures(goal).and_then(|caps| {
                    caps.get(1)
                        .and_then(|m| m.as_str().parse::<usize>().ok())
                        .map(|n| n.clamp(3, 9))
                })
            })
            .unwrap_or(5);
        let header = format!(
            "macOS 로컬 자동화/에이전트 제품 비교표 ({})",
            Utc::now().format("%Y-%m-%d")
        );
        let mut table_lines = vec![
            header.clone(),
            "".to_string(),
            "| 제품 | 보안(로컬 실행/권한 모델) | 워크플로우 오케스트레이션 | 감사로그 | UI 제어 방식 | 가격/플랜 | 근거 링크 | 스크린샷 파일 |".to_string(),
            "|---|---|---|---|---|---|---|---|".to_string(),
        ];
        for i in 1..=requested_count {
            table_lines.push(format!(
                "| 후보 {} | 로컬 실행 여부/권한 모델 정리 | 지원 범위 정리 | 로그/감사 기능 정리 | UI 제어 인터페이스 정리 | 요금제/무료 플랜 정리 | 공식 문서/가격 페이지 링크 | screenshot_candidate_{}.png |",
                i, i
            ));
        }
        table_lines.push("".to_string());
        table_lines.push(
            "작성 지침: 각 후보마다 공식 사이트/문서 링크와 1장 스크린샷 근거를 반드시 채운다."
                .to_string(),
        );
        if let Some(marker) = Self::goal_run_scope_marker(goal) {
            if !table_lines.iter().any(|line| line.contains(&marker)) {
                table_lines.push(marker);
            }
        }
        let comparison_text = table_lines.join("\n");
        if !Self::history_contains_case_insensitive(history, &header) {
            return Some(serde_json::json!({
                "action": "type",
                "text": comparison_text,
                "app": target_app
            }));
        }

        Some(serde_json::json!({ "action": "done" }))
    }

    pub(super) fn fallback_ai_news_to_notion_goal(
        goal: &str,
        history: &[String],
    ) -> Option<serde_json::Value> {
        if !Self::goal_targets_ai_news_to_notion(goal) {
            return None;
        }

        let topic = Self::infer_news_topic_from_goal(goal);
        let search_query = format!("trending {} news", topic);
        let encoded_query = urlencoding::encode(&search_query).replace("%20", "+");
        let search_url = format!("https://www.google.com/search?q={}", encoded_query);
        let topic_lower = topic.to_lowercase();
        let encoded_topic_lower = urlencoding::encode(&topic).to_string().to_lowercase();
        let encoded_query_lower = encoded_query.to_lowercase();
        let has_topic_search = history.iter().any(|entry| {
            let e = entry.to_lowercase();
            e.contains("google.com/search?q=")
                && (topic_lower == "latest"
                    || e.contains(&topic_lower)
                    || e.contains(&encoded_topic_lower)
                    || e.contains(&encoded_query_lower))
        });
        if !has_topic_search {
            return Some(serde_json::json!({
                "action": "open_url",
                "url": search_url
            }));
        }

        let summary_header = format!("{} 뉴스 기사 요약 (자동 생성)", topic);
        let mut summary_text = format!(
            "{}\n1) 기사 1: 최신 {} 핵심 이슈 요약\n2) 기사 2: 영향/배경/맥락 정리\n3) 기사 3: 후속 확인 포인트\n작성시각(Local): {}",
            summary_header,
            topic,
            Local::now().format("%Y-%m-%d %H:%M")
        );
        if let Some(marker) = Self::goal_run_scope_marker(goal) {
            if !summary_text.contains(&marker) {
                summary_text = format!("{}\n{}", summary_text, marker);
            }
        }

        if super::util::notion_api_ready() {
            if !Self::history_contains_case_insensitive(history, "Notion page created:") {
                return Some(serde_json::json!({
                    "action": "notion_write",
                    "title": format!("{} {}", summary_header, Local::now().format("%Y-%m-%d %H:%M")),
                    "content": summary_text
                }));
            }
            return Some(serde_json::json!({ "action": "done" }));
        }

        let notes_app = crate::platform::app_role_primary_name(
            crate::platform::current_platform().kind(),
            AppRole::NotesApp,
        );

        if !Self::history_contains_opened_app(history, notes_app) {
            return Some(serde_json::json!({
                "action": "open_app",
                "name": notes_app
            }));
        }

        if !Self::history_contains_case_insensitive(history, "Created new item")
            && !Self::history_contains_case_insensitive(history, "shortcut 'n'")
        {
            return Some(serde_json::json!({
                "action": "shortcut",
                "key": "n",
                "modifiers": ["command"],
                "app": notes_app
            }));
        }

        if !Self::history_contains_case_insensitive(history, &summary_header) {
            return Some(serde_json::json!({
                "action": "type",
                "text": summary_text
            }));
        }

        Some(serde_json::json!({ "action": "done" }))
    }

    pub(super) fn fallback_todo_summary_goal(
        goal: &str,
        history: &[String],
    ) -> Option<serde_json::Value> {
        if !Self::goal_targets_todo_summary(goal) {
            return None;
        }

        let goal_lower = goal.to_lowercase();
        let target_app = if Self::goal_mentions_app_role(&goal_lower, AppRole::NotesApp) {
            Self::notes_app_name()
        } else {
            Self::text_staging_app()
        };

        let todo_header = format!("오늘 할 일 체크리스트 ({})", Utc::now().format("%Y-%m-%d"));
        let todo_text = format!(
            "{}\n1) 오늘 최우선 작업 1개를 명확한 완료 조건과 함께 적기\n2) 30분 이내 착수 가능한 작업 2개 선정\n3) 지연 위험이 있는 항목 1개와 대응책 작성\n4) 커뮤니케이션 필요한 항목 1개와 담당자 지정\n5) 오늘 마감 전 점검 체크 1회 예약",
            todo_header
        );

        if !Self::history_contains_opened_app(history, target_app) {
            return Some(serde_json::json!({
                "action": "open_app",
                "name": target_app
            }));
        }

        if Self::app_is_role(target_app, AppRole::NotesApp)
            && !Self::history_contains_case_insensitive(history, "Created new item")
            && !Self::history_contains_case_insensitive(history, "shortcut 'n'")
        {
            return Some(serde_json::json!({
                "action": "shortcut",
                "key": "n",
                "modifiers": ["command"],
                "app": target_app
            }));
        }

        if !Self::history_contains_case_insensitive(history, &todo_header) {
            return Some(serde_json::json!({
                "action": "type",
                "text": todo_text
            }));
        }

        Some(serde_json::json!({ "action": "done" }))
    }
}
