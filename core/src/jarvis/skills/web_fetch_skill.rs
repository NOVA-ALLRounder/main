// WebFetchSkill - ???怨쀬뵠??鈺곌퀬??(雅뚯눊?, ??륁몛, URL fetch)
// Yahoo Finance + ExchangeRate-API (?얜?利? API ???븍뜇釉??

use super::{Skill, SkillContext, SkillResult};
use crate::jarvis::skills::metadata::*;
use async_trait::async_trait;
use serde_json::json;

pub struct WebFetchSkill {
    client: reqwest::Client,
}

impl WebFetchSkill {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl Default for WebFetchSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for WebFetchSkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "web_fetch".to_string(),
            description: "???怨쀬뵠??鈺곌퀬?? 雅뚯눊?, ??륁몛, URL fetch".to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "stock_price".to_string(),
                "exchange_rate".to_string(),
                "fetch_url".to_string(),
            ],
            requirements: SkillRequirements::default(), // ?紐? API ???븍뜇釉??
            tags: vec!["finance".to_string(), "web".to_string(), "data".to_string()],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        // ??湲?????揶쎛??(?紐? ??뤵????곸벉)
        EligibilityResult::eligible()
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        log::info!(
            "WebFetchSkill executing action: {} (session: {})",
            ctx.action,
            ctx.session_key
        );

        match ctx.action.as_str() {
            "stock_price" => self.stock_price(ctx).await,
            "exchange_rate" => self.exchange_rate(ctx).await,
            "fetch_url" => self.fetch_url(ctx).await,
            _ => SkillResult::error(format!("Unknown action: {}", ctx.action)),
        }
    }
}

impl WebFetchSkill {
    /// Yahoo Finance?癒?퐣 雅뚯눊? 鈺곌퀬??
    async fn stock_price(&self, ctx: SkillContext) -> SkillResult {
        let symbol = match ctx.params.get("symbol").and_then(|v| v.as_str()) {
            Some(s) => s.to_uppercase(),
            None => return SkillResult::error("'symbol' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??(?? AAPL, TSLA)"),
        };

        // Yahoo Finance v8 API (?⑤벀而?
        let url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=5d",
            urlencoding::encode(&symbol)
        );

        let response = match self.client.get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => return SkillResult::error(format!("雅뚯눊? 鈺곌퀬????쎈솭: {}", e)),
        };

        if !response.status().is_success() {
            return SkillResult::error(format!(
                "Yahoo Finance API ??살첒: HTTP {}",
                response.status()
            ));
        }

        let data: serde_json::Value = match response.json().await {
            Ok(d) => d,
            Err(e) => return SkillResult::error(format!("?臾먮뼗 ???뼓 ??쎈솭: {}", e)),
        };

        // Parse Yahoo Finance response
        let result = &data["chart"]["result"];
        if result.is_null() || !result.is_array() || result.as_array().unwrap().is_empty() {
            return SkillResult::error(format!("'{}' ?ル굝???筌≪뼚??????곷뮸??덈뼄", symbol));
        }

        let meta = &result[0]["meta"];
        let current_price = meta["regularMarketPrice"].as_f64().unwrap_or(0.0);
        let previous_close = meta["chartPreviousClose"].as_f64().unwrap_or(0.0);
        let currency = meta["currency"].as_str().unwrap_or("USD");

        let change = current_price - previous_close;
        let change_pct = if previous_close > 0.0 {
            (change / previous_close) * 100.0
        } else {
            0.0
        };

        let direction = if change >= 0.0 { "+" } else { "" };

        SkillResult::success_with_data(
            format!(
                "{}: {:.2} {} ({}{:.2}, {}{:.2}%)",
                symbol, current_price, currency, direction, change, direction, change_pct
            ),
            json!({
                "symbol": symbol,
                "price": current_price,
                "previous_close": previous_close,
                "change": change,
                "change_percent": change_pct,
                "currency": currency,
            }),
        )
    }

    /// ??륁몛 鈺곌퀬??(ExchangeRate-API ?얜?利??遺얜굡?????
    async fn exchange_rate(&self, ctx: SkillContext) -> SkillResult {
        let from = ctx
            .params
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or("USD")
            .to_uppercase();
        let to = ctx
            .params
            .get("to")
            .and_then(|v| v.as_str())
            .unwrap_or("KRW")
            .to_uppercase();
        let amount = ctx
            .params
            .get("amount")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);

        // Open ExchangeRate API (?얜?利? ???븍뜇釉??
        let url = format!(
            "https://open.er-api.com/v6/latest/{}",
            urlencoding::encode(&from)
        );

        let response = match self.client.get(&url).send().await {
            Ok(resp) => resp,
            Err(e) => return SkillResult::error(format!("??륁몛 鈺곌퀬????쎈솭: {}", e)),
        };

        if !response.status().is_success() {
            return SkillResult::error(format!(
                "ExchangeRate API ??살첒: HTTP {}",
                response.status()
            ));
        }

        let data: serde_json::Value = match response.json().await {
            Ok(d) => d,
            Err(e) => return SkillResult::error(format!("?臾먮뼗 ???뼓 ??쎈솭: {}", e)),
        };

        let rates = &data["rates"];
        let rate = match rates[&to].as_f64() {
            Some(r) => r,
            None => return SkillResult::error(format!("'{}' ???넅??筌≪뼚??????곷뮸??덈뼄", to)),
        };

        let converted = amount * rate;

        SkillResult::success_with_data(
            format!(
                "{:.2} {} = {:.2} {} (1 {} = {:.4} {})",
                amount, from, converted, to, from, rate, to
            ),
            json!({
                "from": from,
                "to": to,
                "amount": amount,
                "rate": rate,
                "converted": converted,
            }),
        )
    }

    /// URL?癒?퐣 ??용뮞??揶쎛?紐꾩궎疫?(揶쏄쑬???遺용튋??
    async fn fetch_url(&self, ctx: SkillContext) -> SkillResult {
        let url = match ctx.params.get("url").and_then(|v| v.as_str()) {
            Some(u) => u.to_string(),
            None => return SkillResult::error("'url' ???뵬沃섎챸苑ｅ첎? ?袁⑹뒄??몃빍??),
        };

        let max_length = ctx
            .params
            .get("max_length")
            .and_then(|v| v.as_u64())
            .unwrap_or(5000) as usize;

        let response = match self.client.get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => return SkillResult::error(format!("URL fetch ??쎈솭: {}", e)),
        };

        let status = response.status().as_u16();
        if !response.status().is_success() {
            return SkillResult::error(format!("HTTP {} ??살첒", status));
        }

        let body = match response.text().await {
            Ok(text) => text,
            Err(e) => return SkillResult::error(format!("?臾먮뼗 ??꾨┛ ??쎈솭: {}", e)),
        };

        // ??용뮞?紐껋춸 ?곕뗄??(揶쏄쑬???HTML strip)
        let text = strip_html_tags(&body);
        let truncated = if text.len() > max_length {
            format!("{}...(truncated)", &text[..max_length])
        } else {
            text.clone()
        };

        SkillResult::success_with_data(
            format!("URL fetch ?袁⑥┷ ({}??", truncated.len()),
            json!({
                "url": url,
                "status": status,
                "content": truncated,
                "original_length": body.len(),
            }),
        )
    }
}

/// 揶쏄쑬???HTML ??볥젃 ??볤탢
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let in_script = false;

    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
            }
            '>' => {
                in_tag = false;
            }
            _ if in_tag => {
                // Check for script/style tags
                // Simple heuristic - skip content inside script tags
            }
            _ if !in_script => {
                result.push(ch);
            }
            _ => {}
        }
    }

    // Clean up whitespace
    result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata() {
        let skill = WebFetchSkill::new();
        let meta = skill.metadata();
        assert_eq!(meta.name, "web_fetch");
        assert_eq!(meta.actions.len(), 3);
        assert!(meta.actions.contains(&"stock_price".to_string()));
        assert!(meta.actions.contains(&"exchange_rate".to_string()));
        assert!(meta.actions.contains(&"fetch_url".to_string()));
    }

    #[test]
    fn test_always_eligible() {
        let skill = WebFetchSkill::new();
        assert!(skill.check_eligibility().eligible);
    }

    #[test]
    fn test_strip_html_tags() {
        let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let text = strip_html_tags(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
        assert!(!text.contains("<"));
    }

    #[test]
    fn test_strip_html_preserves_plain_text() {
        let text = "Just plain text";
        assert_eq!(strip_html_tags(text), "Just plain text");
    }
}
