use crate::controller::actions::{ActionRunner, AiNewsItem};
use anyhow::{anyhow, Result};

impl ActionRunner {
    pub(in crate::controller::actions) fn goal_targets_ai_news_to_notion(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        let asks_news = lower.contains("news")
            || lower.contains("뉴스")
            || lower.contains("헤드라인")
            || lower.contains("headline")
            || lower.contains("기사");
        let asks_summary = lower.contains("요약")
            || lower.contains("summary")
            || lower.contains("정리")
            || lower.contains("선정")
            || lower.contains("digest");
        let asks_notion = lower.contains("notion") || lower.contains("노션");
        asks_news && asks_summary && asks_notion
    }

    pub(in crate::controller::actions) fn infer_news_topic_from_goal(goal: &str) -> String {
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

    pub(in crate::controller::actions) fn infer_news_item_count(goal: &str) -> usize {
        let count_re = regex::Regex::new(r"(?i)\b([1-9]|10)\s*(?:개|items?|articles?)").ok();
        if let Some(re) = count_re {
            if let Some(caps) = re.captures(goal) {
                if let Some(raw) = caps.get(1) {
                    if let Ok(parsed) = raw.as_str().parse::<usize>() {
                        return parsed.clamp(3, 10);
                    }
                }
            }
        }
        5
    }

    pub(in crate::controller::actions) async fn fetch_google_ai_news(
        topic: &str,
        limit: usize,
    ) -> Result<Vec<AiNewsItem>> {
        let topic = if topic.trim().is_empty() {
            "latest".to_string()
        } else {
            topic.trim().to_string()
        };
        let query = if topic.eq_ignore_ascii_case("latest") {
            "trending latest news".to_string()
        } else {
            format!("trending {} news", topic)
        };
        let encoded_query = urlencoding::encode(&query).replace("%20", "+");
        let has_korean = topic.chars().any(|c| ('가'..='힣').contains(&c));
        let (hl, gl, ceid) = if has_korean {
            ("ko", "KR", "KR:ko")
        } else {
            ("en-US", "US", "US:en")
        };
        let url = format!(
            "https://news.google.com/rss/search?q={}&hl={}&gl={}&ceid={}",
            encoded_query, hl, gl, ceid
        );
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let body = client.get(url).send().await?.text().await?;

        let item_re = regex::Regex::new(r"(?is)<item>(.*?)</item>")?;
        let mut seen_links = std::collections::HashSet::new();
        let mut out = Vec::new();

        for cap in item_re.captures_iter(&body) {
            let item_block = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let title = Self::extract_xml_tag_text(item_block, "title").unwrap_or_default();
            let link = Self::extract_xml_tag_text(item_block, "link").unwrap_or_default();
            let pub_date = Self::extract_xml_tag_text(item_block, "pubDate").unwrap_or_default();
            let desc_raw =
                Self::extract_xml_tag_text(item_block, "description").unwrap_or_default();
            let desc = Self::strip_html_tags(&desc_raw);

            if title.is_empty() || link.is_empty() {
                continue;
            }
            if !seen_links.insert(link.clone()) {
                continue;
            }

            let mut summary = if desc.is_empty() {
                format!("{}의 핵심 내용을 간단히 정리한 기사입니다.", title)
            } else {
                desc
            };
            if summary.chars().count() > 220 {
                summary = summary.chars().take(220).collect::<String>() + "...";
            }

            out.push(AiNewsItem {
                title,
                link,
                summary,
                published_at: pub_date,
            });
            if out.len() >= limit {
                break;
            }
        }

        if out.is_empty() {
            return Err(anyhow!("google news rss returned no items"));
        }
        Ok(out)
    }

    pub(in crate::controller::actions) fn build_ai_news_digest(
        topic: &str,
        items: &[AiNewsItem],
        marker: Option<&str>,
    ) -> String {
        fn local_timestamp_string() -> String {
            chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S %:z")
                .to_string()
        }

        fn normalize_pub_date_to_local(raw: &str) -> String {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return String::new();
            }
            if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(trimmed) {
                return dt
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S %:z")
                    .to_string();
            }
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
                return dt
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S %:z")
                    .to_string();
            }
            trimmed.to_string()
        }

        let topic = if topic.trim().is_empty() {
            "최신"
        } else {
            topic.trim()
        };
        let mut lines = vec![
            format!("{} 뉴스 기사 요약 (실기사 기반)", topic),
            format!("작성시각(Local): {}", local_timestamp_string()),
            "".to_string(),
        ];
        for (idx, item) in items.iter().enumerate() {
            lines.push(format!("{}. {}", idx + 1, item.title));
            lines.push(format!("링크: {}", item.link));
            if !item.published_at.trim().is_empty() {
                lines.push(format!(
                    "게시일(Local): {}",
                    normalize_pub_date_to_local(&item.published_at)
                ));
            }
            lines.push("요약:".to_string());
            lines.push(format!("- 핵심: {}", item.summary));
            lines.push(format!(
                "- 맥락: '{}' 관련 최신 동향을 다루는 기사",
                item.title
            ));
            lines.push("- 시사점: 실제 도입/정책/시장 반응을 후속 확인 필요".to_string());
            lines.push("".to_string());
        }
        if let Some(scope) = marker {
            if !scope.trim().is_empty() {
                lines.push(scope.trim().to_string());
            }
        }
        lines.join("\n")
    }
}
