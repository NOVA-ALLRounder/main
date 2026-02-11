// SearchSkill - ??野꺜??(DuckDuckGo, API ???븍뜇釉??
// 筌〓㈇?? skills-main 10e9928a/duckduckgo-search
// DuckDuckGo Instant Answer API + HTML ???뼓

use super::{Skill, SkillContext, SkillResult};
use crate::jarvis::skills::metadata::*;
use async_trait::async_trait;
use serde_json::json;

pub struct SearchSkill {
    client: reqwest::Client,
}

impl SearchSkill {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl Default for SearchSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for SearchSkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "search".to_string(),
            description: "??野꺜?? DuckDuckGo (API ???븍뜇釉?? ?袁⑥뵬??苡??癰귣똾??".to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "web".to_string(),
                "instant".to_string(),
            ],
            requirements: SkillRequirements::default(),
            tags: vec!["search".to_string(), "web".to_string()],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        EligibilityResult::eligible()
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        log::info!("SearchSkill executing action: {}", ctx.action);

        match ctx.action.as_str() {
            "web" => self.web_search(ctx).await,
            "instant" => self.instant_answer(ctx).await,
            _ => SkillResult::error(format!("Unknown search action: {}", ctx.action)),
        }
    }
}

impl SearchSkill {
    /// ??野꺜??(DuckDuckGo HTML 野껉퀗?????뼓)
    async fn web_search(&self, ctx: SkillContext) -> SkillResult {
        let query = match ctx.params.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return SkillResult::error("'query' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
        };
        let max_results = ctx
            .params
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        // DuckDuckGo HTML lite (野꺜??野껉퀗????륁뵠筌왖)
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        let response = match self
            .client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return SkillResult::error(format!("野꺜????쎈솭: {}", e)),
        };

        if !response.status().is_success() {
            return SkillResult::error(format!("DuckDuckGo ??살첒: HTTP {}", response.status()));
        }

        let html = match response.text().await {
            Ok(t) => t,
            Err(e) => return SkillResult::error(format!("?臾먮뼗 ??꾨┛ ??쎈솭: {}", e)),
        };

        // HTML?癒?퐣 野꺜??野껉퀗???곕뗄??
        let results = parse_ddg_results(&html, max_results);

        if results.is_empty() {
            return SkillResult::success_with_data(
                format!("'{}' 野꺜??野껉퀗?드첎? ??곷뮸??덈뼄", query),
                json!({ "query": query, "results": [], "count": 0 }),
            );
        }

        let count = results.len();
        let summary: Vec<String> = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                format!(
                    "{}. {} - {}",
                    i + 1,
                    r["title"].as_str().unwrap_or(""),
                    r["snippet"].as_str().unwrap_or("")
                )
            })
            .collect();

        SkillResult::success_with_data(
            format!("'{}' 野꺜??野껉퀗??{}椰?\n{}", query, count, summary.join("\n")),
            json!({ "query": query, "results": results, "count": count }),
        )
    }

    /// DuckDuckGo Instant Answer API (?닌듼?遺얜쭆 ???)
    async fn instant_answer(&self, ctx: SkillContext) -> SkillResult {
        let query = match ctx.params.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return SkillResult::error("'query' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
        };

        // DuckDuckGo Instant Answer API (?⑤벊?? ?얜?利?
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            urlencoding::encode(query)
        );

        let response = match self
            .client
            .get(&url)
            .header("User-Agent", "SteerJARVIS/1.0")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return SkillResult::error(format!("Instant Answer ??쎈솭: {}", e)),
        };

        let data: serde_json::Value = match response.json().await {
            Ok(d) => d,
            Err(e) => return SkillResult::error(format!("?臾먮뼗 ???뼓 ??쎈솭: {}", e)),
        };

        // Abstract (?袁り텕??곕탵?????遺용튋)
        let abstract_text = data["AbstractText"].as_str().unwrap_or("");
        let abstract_source = data["AbstractSource"].as_str().unwrap_or("");
        let abstract_url = data["AbstractURL"].as_str().unwrap_or("");

        // Answer (筌욊낯?????)
        let answer = data["Answer"].as_str().unwrap_or("");

        // Definition
        let definition = data["Definition"].as_str().unwrap_or("");

        // Related Topics
        let related: Vec<serde_json::Value> = data["RelatedTopics"]
            .as_array()
            .map(|topics| {
                topics
                    .iter()
                    .take(5)
                    .filter_map(|t| {
                        let text = t["Text"].as_str()?;
                        let url = t["FirstURL"].as_str().unwrap_or("");
                        Some(json!({"text": text, "url": url}))
                    })
                    .collect()
            })
            .unwrap_or_default();

        // 揶쎛???醫롮뒠???臾먮뼗 ?醫뤾문
        let best_answer = if !answer.is_empty() {
            answer.to_string()
        } else if !abstract_text.is_empty() {
            abstract_text.to_string()
        } else if !definition.is_empty() {
            definition.to_string()
        } else if !related.is_empty() {
            related
                .iter()
                .take(3)
                .filter_map(|r| r["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            format!("'{}' ??????筌앸맩???????筌≪뼚??????곷뮸??덈뼄. 'web' 野꺜??깆뱽 ??뺣즲????紐꾩뒄.", query)
        };

        SkillResult::success_with_data(
            best_answer,
            json!({
                "query": query,
                "answer": answer,
                "abstract": abstract_text,
                "abstract_source": abstract_source,
                "abstract_url": abstract_url,
                "definition": definition,
                "related_topics": related,
            }),
        )
    }
}

/// DuckDuckGo HTML lite 野꺜??野껉퀗?????뼓
fn parse_ddg_results(html: &str, max_results: usize) -> Vec<serde_json::Value> {
    let mut results = Vec::new();

    // DuckDuckGo HTML lite 野껉퀗?????쉘:
    // <a rel="nofollow" class="result__a" href="URL">TITLE</a>
    // <a class="result__snippet" href="URL">SNIPPET</a>
    let mut i = 0;
    let bytes = html.as_bytes();

    while i < bytes.len() && results.len() < max_results {
        // "result__a" ?????筌≪뼐由?
        if let Some(pos) = html[i..].find("class=\"result__a\"") {
            let abs_pos = i + pos;

            // href ?곕뗄??
            let href = extract_attr(&html[..abs_pos + 200.min(html.len() - abs_pos)], abs_pos, "href");

            // ??볥젃 ??곸뒠 (title) ?곕뗄??
            let title = extract_tag_content(html, abs_pos);

            // snippet 筌≪뼐由?(揶쏆늿? 野껉퀗???됰뗀以???
            let snippet_search_end = (abs_pos + 2000).min(html.len());
            let snippet = if let Some(snip_pos) = html[abs_pos..snippet_search_end].find("result__snippet") {
                let snip_abs = abs_pos + snip_pos;
                extract_tag_content(html, snip_abs)
            } else {
                String::new()
            };

            if !title.is_empty() && !href.is_empty() {
                // DuckDuckGo redirect URL?癒?퐣 ??쇱젫 URL ?곕뗄??
                let actual_url = if href.contains("uddg=") {
                    href.split("uddg=")
                        .nth(1)
                        .and_then(|u| urlencoding::decode(u).ok())
                        .map(|u| u.into_owned())
                        .unwrap_or(href.clone())
                } else {
                    href
                };

                results.push(json!({
                    "title": clean_html_entities(&title),
                    "url": actual_url,
                    "snippet": clean_html_entities(&snippet),
                }));
            }

            i = abs_pos + 100;
        } else {
            break;
        }
    }

    results
}

/// HTML ??볥젃 ??곸뒠 ?곕뗄??(揶쏄쑬??甕곌쑴??
fn extract_tag_content(html: &str, start: usize) -> String {
    // '>' 筌≪뼐由?(??볥젃 ??
    if let Some(tag_end) = html[start..].find('>') {
        let content_start = start + tag_end + 1;
        // '<' 筌≪뼐由?(??쇱벉 ??볥젃 ??뽰삂)
        if let Some(next_tag) = html[content_start..].find('<') {
            let content = &html[content_start..content_start + next_tag];
            return content.trim().to_string();
        }
    }
    String::new()
}

/// HTML ??욧쉐 ?곕뗄??
fn extract_attr(html: &str, around_pos: usize, attr_name: &str) -> String {
    // around_pos 域뱀눘荑?癒?퐣 attr_name="value" ???쉘 筌≪뼐由?
    let search_start = if around_pos > 500 { around_pos - 500 } else { 0 };
    let search_region = &html[search_start..around_pos + 200.min(html.len() - around_pos)];

    let pattern = format!("{}=\"", attr_name);
    if let Some(attr_start) = search_region.rfind(&pattern) {
        let value_start = attr_start + pattern.len();
        if let Some(value_end) = search_region[value_start..].find('"') {
            return search_region[value_start..value_start + value_end].to_string();
        }
    }
    String::new()
}

/// HTML ?酉????????
fn clean_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("<b>", "")
        .replace("</b>", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata() {
        let skill = SearchSkill::new();
        let meta = skill.metadata();
        assert_eq!(meta.name, "search");
        assert_eq!(meta.actions.len(), 2);
    }

    #[test]
    fn test_always_eligible() {
        let skill = SearchSkill::new();
        assert!(skill.check_eligibility().eligible);
    }

    #[test]
    fn test_clean_html_entities() {
        assert_eq!(clean_html_entities("foo &amp; bar"), "foo & bar");
        assert_eq!(clean_html_entities("<b>bold</b>"), "bold");
        assert_eq!(clean_html_entities("A &gt; B"), "A > B");
    }

    #[test]
    fn test_extract_tag_content() {
        let html = r#"<a class="test" href="url">Hello World</a>"#;
        let content = extract_tag_content(html, 0);
        assert_eq!(content, "Hello World");
    }
}
