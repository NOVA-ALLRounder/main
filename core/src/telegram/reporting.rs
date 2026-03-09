use super::TelegramBot;
use crate::controller::planner::RunGoalOutcome;
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone)]
pub(crate) struct RunResultContext {
    pub(crate) links: Vec<String>,
    pub(crate) highlights: Vec<String>,
}

impl TelegramBot {
    pub(crate) fn build_run_report(
        outcome: &RunGoalOutcome,
        stage_runs: &[crate::db::TaskStageRunRecord],
        assertions: &[crate::db::TaskStageAssertionRecord],
        goal: &str,
        result_context: &RunResultContext,
    ) -> String {
        fn truncate_chars(s: &str, max_chars: usize) -> String {
            let mut out = String::new();
            for (idx, ch) in s.chars().enumerate() {
                if idx >= max_chars {
                    out.push_str("...");
                    break;
                }
                out.push(ch);
            }
            out
        }

        let summary = outcome.summary.clone().unwrap_or_else(|| "n/a".to_string());

        if outcome.business_complete {
            let mut lines = Vec::new();
            lines.push("상태: ✅ 성공".to_string());
            lines.push(format!("요청: {}", truncate_chars(goal.trim(), 120)));
            if !summary.is_empty() && summary != "n/a" {
                lines.push(format!("결과: {}", truncate_chars(&summary, 160)));
            }
            if !result_context.highlights.is_empty() {
                lines.push("핵심 요약:".to_string());
                for item in result_context.highlights.iter().take(5) {
                    lines.push(format!("- {}", truncate_chars(item, 240)));
                }
            }
            if !result_context.links.is_empty() {
                lines.push("노션 링크:".to_string());
                for link in result_context.links.iter().take(4) {
                    lines.push(format!("- {}", link));
                }
            }
            lines.push(format!("run_id: {}", outcome.run_id));
            lines.push("다음 조치:".to_string());
            lines.push("- 추가 요청 실행 가능".to_string());
            return lines.join("\n");
        }

        let mut latest_stage_map: BTreeMap<(i64, String), crate::db::TaskStageRunRecord> =
            BTreeMap::new();
        for stage in stage_runs {
            let key = (stage.stage_order, stage.stage_name.clone());
            match latest_stage_map.get(&key) {
                Some(prev) if prev.id >= stage.id => {}
                _ => {
                    latest_stage_map.insert(key, stage.clone());
                }
            }
        }
        let latest_stages: Vec<crate::db::TaskStageRunRecord> =
            latest_stage_map.into_values().collect();
        let stage_done_count = latest_stages
            .iter()
            .filter(|s| s.status.eq_ignore_ascii_case("completed"))
            .count();
        let stage_total = latest_stages.len();

        let mut lines = Vec::new();
        lines.push(if outcome.business_complete {
            "상태: ✅ 성공".to_string()
        } else {
            "상태: ❌ 실패".to_string()
        });
        lines.push(format!("run_id: {}", outcome.run_id));
        lines.push(format!("요약: {}", summary));
        lines.push("판정:".to_string());
        lines.push(format!("- planner_complete={}", outcome.planner_complete));
        lines.push(format!(
            "- execution_complete={}",
            outcome.execution_complete
        ));
        lines.push(format!("- business_complete={}", outcome.business_complete));
        lines.push(format!("- final_status={}", outcome.status));

        if !latest_stages.is_empty() {
            lines.push(format!("단계: {}/{} 완료", stage_done_count, stage_total));
            for stage in latest_stages.iter().take(8) {
                let details = stage.details.clone().unwrap_or_default();
                let short_details = if details.chars().count() > 120 {
                    truncate_chars(&details, 120)
                } else {
                    details
                };
                if short_details.is_empty() {
                    lines.push(format!(
                        "- {}.{}={}",
                        stage.stage_order, stage.stage_name, stage.status
                    ));
                } else {
                    lines.push(format!(
                        "- {}.{}={} ({})",
                        stage.stage_order, stage.stage_name, stage.status, short_details
                    ));
                }
            }
        }

        let mut failed_assertion_map: BTreeMap<
            (String, String),
            crate::db::TaskStageAssertionRecord,
        > = BTreeMap::new();
        for assertion in assertions.iter().filter(|a| !a.passed) {
            let key = (
                assertion.stage_name.clone(),
                assertion.assertion_key.clone(),
            );
            match failed_assertion_map.get(&key) {
                Some(prev) if prev.id >= assertion.id => {}
                _ => {
                    failed_assertion_map.insert(key, assertion.clone());
                }
            }
        }
        let failed_assertions: Vec<crate::db::TaskStageAssertionRecord> =
            failed_assertion_map.into_values().collect();
        if assertions.is_empty() {
            lines.push("검증: assertion 없음".to_string());
        } else if failed_assertions.is_empty() {
            lines.push(format!("검증: assertions 통과 ({})", assertions.len()));
        } else {
            lines.push(format!(
                "검증: assertions 실패 {}/{}",
                failed_assertions.len(),
                assertions.len()
            ));
            lines.push("실패 근거:".to_string());
            for assertion in failed_assertions.iter().take(6) {
                let evidence = assertion.evidence.clone().unwrap_or_default();
                let short_evidence = if evidence.chars().count() > 140 {
                    truncate_chars(&evidence, 140)
                } else {
                    evidence
                };
                lines.push(format!(
                    "- {}.{} expected={} actual={} evidence={}",
                    assertion.stage_name,
                    assertion.assertion_key,
                    assertion.expected,
                    assertion.actual,
                    if short_evidence.is_empty() {
                        "n/a".to_string()
                    } else {
                        short_evidence
                    }
                ));
            }
        }

        if !result_context.links.is_empty() {
            lines.push("결과 링크:".to_string());
            for link in result_context.links.iter().take(4) {
                lines.push(format!("- {}", link));
            }
        }

        lines.push("다음 조치:".to_string());
        if outcome.business_complete {
            lines.push("- 추가 요청 실행 가능".to_string());
        } else {
            lines.push("- 실패 근거 항목부터 보강 후 재실행".to_string());
            lines.push("- 동일 요청 재실행 전 front 앱/입력 포커스 확인".to_string());
        }

        lines.join("\n")
    }

    pub(crate) fn collect_result_context(session_key: &str) -> RunResultContext {
        if session_key.trim().is_empty() {
            return RunResultContext::default();
        }
        let _ = crate::session_store::init_session_store();
        let guard = match crate::session_store::get_session_store() {
            Ok(g) => g,
            Err(_) => return RunResultContext::default(),
        };
        let store = match guard.as_ref() {
            Some(s) => s,
            None => return RunResultContext::default(),
        };
        let session = match store.get_latest_by_key(session_key) {
            Some(s) => s,
            None => return RunResultContext::default(),
        };
        Self::extract_result_context_from_steps(&session.steps)
    }

    pub(crate) fn extract_result_context_from_steps(
        steps: &[crate::session_store::SessionStep],
    ) -> RunResultContext {
        let mut ctx = RunResultContext {
            links: Self::extract_result_links_from_steps(steps),
            highlights: Vec::new(),
        };

        for step in steps.iter().rev() {
            if !step.action_type.eq_ignore_ascii_case("notion_write") {
                continue;
            }
            if let Some(data) = step.data.as_ref() {
                if let Some(preview) = data.get("content_preview").and_then(|v| v.as_str()) {
                    ctx.highlights = Self::extract_news_highlights_from_preview(preview);
                }
            }
            break;
        }

        ctx
    }

    pub(crate) fn extract_result_links_from_steps(
        steps: &[crate::session_store::SessionStep],
    ) -> Vec<String> {
        fn push_http_link(out: &mut Vec<String>, raw: &str) {
            let candidate = raw
                .trim()
                .trim_matches(|c| c == '"' || c == '\'' || c == '`');
            if !(candidate.starts_with("https://") || candidate.starts_with("http://")) {
                return;
            }
            if !out.iter().any(|v| v.eq_ignore_ascii_case(candidate)) {
                out.push(candidate.to_string());
            }
        }

        let mut links = Vec::new();
        for step in steps.iter().rev() {
            if let Some(data) = step.data.as_ref() {
                for key in ["page_url", "workflow_url", "execution_url", "run_url"] {
                    if let Some(url) = data.get(key).and_then(|v| v.as_str()) {
                        push_http_link(&mut links, url);
                    }
                }
            }

            if let Some(url) = step.description.strip_prefix("Notion page created: ") {
                push_http_link(&mut links, url);
            }
        }
        links
    }

    pub(crate) fn extract_news_highlights_from_preview(preview: &str) -> Vec<String> {
        fn trim_title_line(line: &str) -> Option<String> {
            let trimmed = line.trim();
            let (num, rest) = trimmed.split_once('.')?;
            if !num.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let title = rest.trim();
            if title.is_empty() {
                None
            } else {
                Some(title.to_string())
            }
        }

        fn shorten(s: &str, max_chars: usize) -> String {
            let mut out = String::new();
            for (idx, ch) in s.chars().enumerate() {
                if idx >= max_chars {
                    out.push_str("...");
                    break;
                }
                out.push(ch);
            }
            out
        }

        let lines: Vec<&str> = preview.lines().collect();
        let mut highlights = Vec::new();
        let mut idx = 0usize;

        while idx < lines.len() && highlights.len() < 5 {
            let current = lines[idx].trim();
            let Some(title) = trim_title_line(current) else {
                idx += 1;
                continue;
            };

            let mut link = String::new();
            let mut core = String::new();
            let mut j = idx + 1;
            while j < lines.len() {
                let next = lines[j].trim();
                if next.is_empty() || trim_title_line(next).is_some() {
                    break;
                }
                if link.is_empty() {
                    if let Some(v) = next.strip_prefix("링크:") {
                        link = v.trim().to_string();
                    }
                }
                if core.is_empty() {
                    if let Some(v) = next.strip_prefix("- 핵심:") {
                        core = v.trim().to_string();
                    }
                }
                j += 1;
            }

            let mut bullet = title;
            if !core.is_empty() {
                bullet.push_str(" — ");
                bullet.push_str(&core);
            }
            if !link.is_empty() {
                bullet.push_str(" (");
                bullet.push_str(&link);
                bullet.push(')');
            }
            highlights.push(shorten(&bullet, 260));
            idx = j.max(idx + 1);
        }

        highlights
    }

    pub async fn improve_message(&self, raw_text: &str) -> String {
        let messages = vec![
            json!({
                "role": "system",
                "content": concat!(
                    "너는 로컬 OS 에이전트 실행 결과를 텔레그램용으로 정리하는 리포터다.\n",
                    "입력은 테스트 시나리오 로그 또는 자연어 요청 실행 요약이다.\n",
                    "입력에 있는 사실만 사용하고 추측하지 마라.\n",
                    "출력은 반드시 한국어로 작성한다.\n",
                    "구분선(---), 장식용 이모지, 불필요한 수식어는 금지한다.\n",
                    "반드시 아래 형식을 정확히 지켜라:\n",
                    "작업: ...\n",
                    "요청: ...\n",
                    "수행: ...\n",
                    "결과: ...\n",
                    "상태: ✅ 성공 또는 ❌ 실패\n",
                    "근거:\n",
                    "- ...\n",
                    "- ...\n",
                    "- ...\n",
                    "- 로그: 파일명\n",
                    "- 캡처: 파일명\n",
                    "근거 불릿은 최소 3개 이상 작성하라.\n",
                    "입력에 로그/캡처 파일명이 있으면 그대로 포함하라.\n",
                    "입력에 'Node evidence:' 또는 '노드 캡처' 항목이 있으면 근거에 반드시 포함하라.\n",
                    "Node evidence가 있으면 최소 2개 이상 불릿으로 유지하라.\n",
                    "노드 캡처 수/노드 캡처 폴더/노드샷 항목이 있으면 삭제하지 마라.\n",
                    "각 줄은 짧고 명확하게 작성하라.\n",
                    "근거 불릿은 입력 원문의 로그 문장을 그대로 또는 최소한으로만 축약해서 작성하라.\n",
                    "입력에 없는 사실(예: 실제 메모 작성 완료, 전송 완료, 저장 완료)을 추정해 쓰지 마라.\n",
                    "open_app 로그만 있으면 '앱 열림' 수준으로만 기술하고 작업 완료를 확대 해석하지 마라.\n",
                    "실패 신호(error, failed, panic, ❌, refused)가 있으면 상태는 ❌ 실패로 작성한다.\n",
                    "정보가 부족하면 '확인 불가'라고 명시한다."
                )
            }),
            json!({
                "role": "user",
                "content": format!("Raw Input: \"{}\"", raw_text)
            }),
        ];

        match self.llm.chat_completion(messages).await {
            Ok(refined) => refined.trim().to_string(),
            Err(_) => raw_text.to_string(),
        }
    }

    pub async fn send_smart_notification(
        &self,
        chat_id: i64,
        raw_text: &str,
    ) -> anyhow::Result<()> {
        let refined_text = self.improve_message(raw_text).await;
        if self
            .send_message_chunked(chat_id, &refined_text)
            .await
            .is_ok()
        {
            return Ok(());
        }
        self.send_message_chunked(chat_id, raw_text).await
    }
}
