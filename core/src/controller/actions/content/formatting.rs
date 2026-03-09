use crate::controller::actions::ActionRunner;
use serde_json::json;

impl ActionRunner {
    pub(in crate::controller::actions) fn notion_paragraph_blocks(
        content: &str,
    ) -> Vec<serde_json::Value> {
        fn text_segment(content: &str, link: Option<&str>) -> serde_json::Value {
            let mut text_obj = json!({ "content": content });
            if let Some(url) = link {
                text_obj["link"] = json!({ "url": url });
            }
            json!({
                "type": "text",
                "text": text_obj
            })
        }

        fn rich_text_with_links(line: &str) -> Vec<serde_json::Value> {
            let mut rich: Vec<serde_json::Value> = Vec::new();
            let Ok(url_re) = regex::Regex::new(r"https?://\S+") else {
                return vec![text_segment(line, None)];
            };

            let mut last = 0usize;
            for m in url_re.find_iter(line) {
                if m.start() > last {
                    let prefix = &line[last..m.start()];
                    if !prefix.is_empty() {
                        rich.push(text_segment(prefix, None));
                    }
                }

                let raw = m.as_str();
                let trimmed = raw.trim_end_matches(|c: char| {
                    matches!(c, ')' | ']' | '}' | '.' | ',' | ';' | ':' | '!' | '?')
                });
                let suffix = &raw[trimmed.len()..];
                if !trimmed.is_empty() {
                    rich.push(text_segment(trimmed, Some(trimmed)));
                }
                if !suffix.is_empty() {
                    rich.push(text_segment(suffix, None));
                }
                last = m.end();
            }

            if last < line.len() {
                let tail = &line[last..];
                if !tail.is_empty() {
                    rich.push(text_segment(tail, None));
                }
            }

            if rich.is_empty() {
                rich.push(text_segment(line, None));
            }
            rich
        }

        let mut blocks: Vec<serde_json::Value> = Vec::new();
        for line in content.lines().take(80) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let clipped: String = trimmed.chars().take(1800).collect();
            blocks.push(json!({
                "object": "block",
                "type": "paragraph",
                "paragraph": {
                    "rich_text": rich_text_with_links(&clipped)
                }
            }));
        }
        if blocks.is_empty() {
            let clipped: String = content.trim().chars().take(1800).collect();
            if !clipped.is_empty() {
                blocks.push(json!({
                    "object": "block",
                    "type": "paragraph",
                    "paragraph": {
                        "rich_text": rich_text_with_links(&clipped)
                    }
                }));
            }
        }
        blocks
    }

    pub(in crate::controller::actions) fn decode_xml_entities(raw: &str) -> String {
        raw.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&#x27;", "'")
            .replace("&nbsp;", " ")
    }

    pub(in crate::controller::actions) fn strip_html_tags(raw: &str) -> String {
        let stripped = if let Ok(re) = regex::Regex::new(r"(?is)<[^>]+>") {
            re.replace_all(raw, " ").to_string()
        } else {
            raw.to_string()
        };
        stripped
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }

    pub(in crate::controller::actions) fn extract_xml_tag_text(
        block: &str,
        tag: &str,
    ) -> Option<String> {
        let open = format!("<{}>", tag);
        let close = format!("</{}>", tag);
        let start = block.find(&open)?;
        let end = block[start + open.len()..].find(&close)?;
        let mut value = block[start + open.len()..start + open.len() + end]
            .trim()
            .to_string();
        if value.starts_with("<![CDATA[") && value.ends_with("]]>") {
            value = value
                .trim_start_matches("<![CDATA[")
                .trim_end_matches("]]>")
                .to_string();
        }
        let decoded = Self::decode_xml_entities(&value).trim().to_string();
        if decoded.is_empty() {
            None
        } else {
            Some(decoded)
        }
    }
}
