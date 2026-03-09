use super::Planner;

impl Planner {
    pub(super) fn infer_news_topic_from_goal(goal: &str) -> String {
        let lower = goal.to_lowercase();
        let topic_map: &[(&[&str], &str)] = &[
            (
                &[
                    "스포츠",
                    "sport",
                    "nba",
                    "nfl",
                    "mlb",
                    "epl",
                    "축구",
                    "야구",
                    "농구",
                ],
                "스포츠",
            ),
            (
                &[
                    "경제", "금융", "finance", "market", "stock", "주식", "증시", "코인",
                ],
                "경제",
            ),
            (
                &[
                    "정치",
                    "politic",
                    "election",
                    "정부",
                    "대통령",
                    "의회",
                    "외교",
                ],
                "정치",
            ),
            (&["과학", "science", "연구", "우주"], "과학"),
            (&["기술", "tech", "it", "startup", "반도체"], "기술"),
            (&["ai", "인공지능", "머신러닝", "생성형"], "AI"),
            (
                &["연예", "엔터", "entertainment", "movie", "music"],
                "엔터테인먼트",
            ),
            (&["건강", "의료", "health", "medicine"], "건강"),
        ];
        for (needles, topic) in topic_map {
            if needles.iter().any(|needle| lower.contains(needle)) {
                return topic.to_string();
            }
        }

        let compact_topic_re =
            regex::Regex::new(r"([가-힣A-Za-z0-9+#.&/\-]{2,40})\s*(?:뉴스|기사|헤드라인)").ok();
        if let Some(re) = compact_topic_re {
            if let Some(captures) = re.captures(goal) {
                if let Some(raw) = captures.get(1) {
                    let candidate = raw
                        .as_str()
                        .trim()
                        .trim_matches(|c: char| c == '"' || c == '\'')
                        .to_string();
                    if !candidate.is_empty()
                        && !["요약", "정리", "선정", "최신", "오늘", "개"]
                            .iter()
                            .any(|w| candidate.eq_ignore_ascii_case(w))
                    {
                        return candidate;
                    }
                }
            }
        }

        "latest".to_string()
    }

    pub(super) fn goal_targets_ai_news_to_notion(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        let asks_news = lower.contains("news")
            || lower.contains("headline")
            || lower.contains("article")
            || lower.contains("기사")
            || lower.contains("트렌드")
            || lower.contains("헤드라인")
            || lower.contains("브리핑")
            || lower.contains("trend")
            || lower.contains("digest")
            || lower.contains("trendy")
            || lower.contains("뉴스");
        let asks_summary = lower.contains("요약")
            || lower.contains("summar")
            || lower.contains("정리")
            || lower.contains("선정")
            || lower.contains("모아")
            || lower.contains("핵심");
        let asks_notion = lower.contains("notion") || lower.contains("노션");
        asks_news && asks_summary && asks_notion
    }

    pub(super) fn goal_targets_todo_summary(goal: &str) -> bool {
        let lower = Self::normalize_text_for_matching(goal);
        let asks_todo = Self::goal_contains_any(
            &lower,
            &[
                "todo",
                "to-do",
                "task",
                "tasks",
                "할 일",
                "할일",
                "체크리스트",
                "업무",
            ],
        );
        let asks_summary_or_list = Self::goal_contains_any(
            &lower,
            &[
                "요약",
                "정리",
                "목록",
                "리스트",
                "만들",
                "작성",
                "summar",
                "list",
                "organize",
            ],
        );
        asks_todo && asks_summary_or_list && !Self::goal_targets_ai_news_to_notion(goal)
    }

    pub(super) fn goal_targets_product_comparison_research(goal: &str) -> bool {
        let lower = Self::normalize_text_for_matching(goal);
        let asks_compare = Self::goal_contains_any(
            &lower,
            &[
                "비교",
                "비교표",
                "표",
                "table",
                "matrix",
                "comparison",
                "compare",
            ],
        );
        let asks_products = Self::goal_contains_any(
            &lower,
            &[
                "product",
                "products",
                "tool",
                "tools",
                "서비스",
                "제품",
                "agent",
                "에이전트",
                "automation",
                "자동화",
                "local",
                "로컬",
                "macos",
            ],
        );
        let asks_research = Self::goal_contains_any(
            &lower,
            &[
                "find",
                "찾아",
                "조사",
                "research",
                "similar",
                "유사",
                "alternative",
                "alternatives",
            ],
        );
        let asks_evidence = Self::goal_contains_any(
            &lower,
            &[
                "스크린샷",
                "screenshot",
                "링크",
                "link",
                "evidence",
                "근거",
                "price",
                "pricing",
                "가격",
                "보안",
                "security",
                "감사로그",
                "audit",
            ],
        );
        asks_compare && asks_products && (asks_research || asks_evidence)
    }

    pub(super) fn infer_product_comparison_search_query(goal: &str) -> String {
        let lower = Self::normalize_text_for_matching(goal);
        let mut seed = if lower.contains("openclaw") {
            "OpenClaw alternatives macOS local agent automation".to_string()
        } else if lower.contains("로컬") || lower.contains("local") {
            "macOS local automation agent tools".to_string()
        } else {
            "macOS automation agent tools".to_string()
        };
        if Self::goal_contains_any(
            &lower,
            &[
                "security",
                "보안",
                "감사로그",
                "audit",
                "price",
                "가격",
                "pricing",
            ],
        ) {
            seed.push_str(" security audit log pricing");
        } else {
            seed.push_str(" comparison");
        }
        seed
    }

    pub(super) fn text_staging_app() -> &'static str {
        match std::env::var("STEER_TEXT_STAGING_APP") {
            Ok(raw) => {
                let v = raw.trim().to_lowercase();
                if v == "notes" || v == "메모" {
                    "Notes"
                } else {
                    "TextEdit"
                }
            }
            Err(_) => "TextEdit",
        }
    }

    pub(super) fn should_use_deterministic_goal_autoplan(goal: &str) -> bool {
        let lower = Self::normalize_text_for_matching(goal);
        if Self::goal_targets_ai_news_to_notion(goal) {
            return true;
        }
        if Self::goal_targets_todo_summary(goal) {
            return true;
        }
        if Self::goal_targets_product_comparison_research(goal) {
            return true;
        }
        if Self::goal_targets_n8n_workflow_request(goal) {
            return true;
        }
        if Self::env_truthy("STEER_FORCE_DETERMINISTIC_GOAL_AUTOPLAN") {
            return true;
        }

        if !Self::env_truthy_default("STEER_DETERMINISTIC_GOAL_AUTOPLAN", true) {
            return false;
        }
        if Self::goal_targets_ai_news_to_notion(goal) {
            return true;
        }

        let apps = Self::ordered_apps_in_goal(goal);
        let text_fragments = Self::extract_goal_text_fragments(goal);
        let inferred_open_app = Self::extract_known_app_from_text(goal);
        let explicit_ops = Self::goal_contains_any(
            &lower,
            &[
                "cmd+",
                "command+",
                "전체 선택",
                "복사",
                "붙여넣",
                "copy",
                "paste",
                "subject",
                "제목",
                "받는 사람",
                "recipient",
                "send",
                "보내기",
                "발송",
            ],
        );

        let direct_delivery_goal =
            Self::goal_requires_mail_send(goal) || Self::goal_requires_telegram_send(goal);
        if direct_delivery_goal && !apps.is_empty() {
            return true;
        }

        let single_textual_write_goal = apps.len() == 1
            && Self::is_textual_app(apps[0])
            && Self::goal_has_write_signal(&lower)
            && !text_fragments.is_empty();
        if single_textual_write_goal {
            return true;
        }

        let simple_open_goal = (apps.len() == 1
            || (apps.is_empty() && inferred_open_app.is_some()))
            && Self::goal_has_open_signal(&lower)
            && !Self::goal_has_write_signal(&lower)
            && !Self::goal_has_payload_tokens(goal);
        if simple_open_goal {
            return true;
        }

        apps.len() >= 2 && text_fragments.len() >= 2 && explicit_ops
    }

    pub(super) fn goal_has_payload_tokens(goal: &str) -> bool {
        !Self::extract_goal_text_fragments(goal).is_empty()
    }

    pub(super) fn goal_has_write_signal(lower: &str) -> bool {
        Self::goal_contains_any(
            lower,
            &[
                "write",
                "작성",
                "입력",
                "써",
                "적어",
                "붙여넣",
                "paste",
                "type",
                "append",
                "기록",
                "escribe",
                "escribir",
                "écris",
                "ecris",
                "rédige",
                "redige",
                "schreib",
                "書",
                "入力して",
                "写",
                "输入",
            ],
        )
    }

    pub(super) fn goal_has_open_signal(lower: &str) -> bool {
        Self::goal_contains_any(
            lower,
            &[
                "open", "launch", "열어", "열고", "실행", "켜", "띄워", "abre", "abrir", "ouvre",
                "ouvrir", "öffne", "oeffne", "開", "打开", "開啟",
            ],
        )
    }

    pub(super) fn goal_has_new_item_signal(lower: &str) -> bool {
        Self::goal_contains_any(
            lower,
            &[
                "new note",
                "new document",
                "새 메모",
                "새 문서",
                "cmd+n",
                "command+n",
            ],
        )
    }

    pub(super) fn goal_requires_notes_write(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        let mentions_notes = lower.contains("notes") || lower.contains("메모");
        mentions_notes
            && (Self::goal_has_write_signal(&lower)
                || Self::goal_has_new_item_signal(&lower)
                || Self::goal_has_payload_tokens(goal))
    }

    pub(super) fn goal_requires_textedit_write(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        let mentions_textedit = lower.contains("textedit") || lower.contains("텍스트에디트");
        mentions_textedit
            && (Self::goal_has_write_signal(&lower)
                || Self::goal_has_new_item_signal(&lower)
                || Self::goal_has_payload_tokens(goal))
    }

    pub(super) fn goal_requires_textedit_save(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        let mentions_textedit = lower.contains("textedit") || lower.contains("텍스트에디트");
        let mentions_save_shortcut = Self::contains_shortcut_token(&lower, "cmd", "s")
            || Self::contains_shortcut_token(&lower, "command", "s");
        let mentions_save =
            Self::goal_contains_any(&lower, &["save", "저장", "파일로 저장", "저장해"])
                || mentions_save_shortcut;
        mentions_textedit && mentions_save
    }

    pub(super) fn contains_shortcut_token(text_lower: &str, modifier: &str, key: &str) -> bool {
        let escaped_modifier = regex::escape(modifier);
        let escaped_key = regex::escape(key);
        let pattern = format!(
            r"(^|[^a-z0-9_+]){}\s*\+\s*{}([^a-z0-9_+]|$)",
            escaped_modifier, escaped_key
        );
        regex::Regex::new(&pattern)
            .map(|re| re.is_match(text_lower))
            .unwrap_or(false)
    }

    pub(super) fn is_textual_app(app: &str) -> bool {
        app.eq_ignore_ascii_case("Notes")
            || app.eq_ignore_ascii_case("TextEdit")
            || app.eq_ignore_ascii_case("Mail")
    }
}
