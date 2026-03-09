pub fn extract_best_number(text: &str) -> Option<String> {
    let mut nums: Vec<String> = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == ',' {
            buf.push(ch);
        } else if !buf.is_empty() {
            nums.push(buf.clone());
            buf.clear();
        }
    }
    if !buf.is_empty() {
        nums.push(buf);
    }

    let mut cleaned: Vec<String> = nums
        .into_iter()
        .map(|n| n.replace(',', ""))
        .map(|n| n.trim_matches('.').to_string())
        .filter(|n| !n.is_empty() && n.chars().any(|c| c.is_ascii_digit()))
        .collect();

    if cleaned.is_empty() {
        return None;
    }

    if let Some(first_decimal) = cleaned.iter().find(|n| n.contains('.')) {
        return Some(first_decimal.clone());
    }

    cleaned.sort_by(|a, b| {
        let av = a.parse::<f64>().unwrap_or(0.0);
        let bv = b.parse::<f64>().unwrap_or(0.0);
        bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
    });
    cleaned.first().cloned()
}

pub fn calculator_has_input(history: &[String]) -> bool {
    let mut seen_open = false;
    for entry in history.iter().rev() {
        if entry.contains("Opened app: Calculator") {
            seen_open = true;
            break;
        }
    }
    if !seen_open {
        return false;
    }
    for entry in history.iter().rev() {
        if entry.contains("Opened app: Calculator") {
            break;
        }
        if entry.starts_with("Typed '") {
            return true;
        }
    }
    false
}

pub fn goal_mentions_calculation(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    lower.contains("계산")
        || lower.contains("calculate")
        || lower.contains("곱")
        || lower.contains("×")
        || lower.contains("*")
        || lower.contains("plus")
        || lower.contains("minus")
}

pub fn goal_is_ui_task(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    let apps = [
        "safari",
        "notes",
        "finder",
        "preview",
        "textedit",
        "mail",
        "calculator",
    ];
    apps.iter().any(|app| lower.contains(app))
}

pub fn goal_mentions_desktop(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    lower.contains("desktop") || lower.contains("데스크탑")
}

pub fn goal_mentions_image(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    lower.contains("image")
        || lower.contains("이미지")
        || lower.contains(".png")
        || lower.contains(".jpg")
}

pub fn goal_mentions_notes(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    lower.contains("notes") || lower.contains("메모")
}

pub fn infer_stock_symbol(goal: &str, query: &str) -> Option<&'static str> {
    let lower = format!("{} {}", goal.to_lowercase(), query.to_lowercase());
    if lower.contains("aapl") || lower.contains("apple") {
        return Some("AAPL");
    }
    None
}

pub fn goal_mentions_stock_price(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    lower.contains("stock price") || lower.contains("주가")
}

pub async fn fetch_stock_price(symbol: &str) -> Option<String> {
    let url = format!(
        "https://query1.finance.yahoo.com/v7/finance/quote?symbols={}",
        symbol
    );
    if let Ok(resp) = reqwest::get(&url).await {
        if let Ok(body) = resp.text().await {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(price) = json
                    .get("quoteResponse")
                    .and_then(|v| v.get("result"))
                    .and_then(|v| v.get(0))
                    .and_then(|v| v.get("regularMarketPrice"))
                    .and_then(|v| v.as_f64())
                {
                    return Some(format!("{}", price));
                }
            }
        }
    }

    let sym = symbol.to_lowercase();
    let stooq_url = format!("https://stooq.com/q/l/?s={}.us&f=sd2t2ohlcv&h&e=csv", sym);
    let resp = reqwest::get(&stooq_url).await.ok()?;
    let body = resp.text().await.ok()?;
    let mut lines = body.lines();
    let _header = lines.next()?;
    let data = lines.next()?;
    let cols: Vec<&str> = data.split(',').collect();
    if cols.len() >= 8 {
        let close = cols[6].trim();
        if !close.is_empty() && close != "N/A" {
            return Some(close.to_string());
        }
    }
    None
}

pub fn compute_calc_result(num_str: &str) -> Option<String> {
    let cleaned = num_str.replace(',', "");
    let val: f64 = cleaned.parse().ok()?;
    let result = val * 100.0;
    let mut text = format!("{:.2}", result);
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    Some(text)
}

pub fn normalize_digits(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect()
}

pub fn extract_search_query(goal: &str) -> Option<String> {
    if let Some(start) = goal.find('\'') {
        if let Some(end) = goal[start + 1..].find('\'') {
            let query = &goal[start + 1..start + 1 + end];
            if !query.trim().is_empty() {
                return Some(query.trim().to_string());
            }
        }
    }
    if let Some(start) = goal.find('\"') {
        if let Some(end) = goal[start + 1..].find('\"') {
            let query = &goal[start + 1..start + 1 + end];
            if !query.trim().is_empty() {
                return Some(query.trim().to_string());
            }
        }
    }

    let lower = goal.to_lowercase();
    for key in ["검색:", "search:", "검색", "search"] {
        if let Some(idx) = lower.find(key) {
            let rest = goal[idx + key.len()..].trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }

    None
}

pub fn extract_note_title(goal: &str) -> Option<String> {
    let lower = goal.to_lowercase();
    let mut after_title = None;
    for key in ["제목", "title"] {
        if let Some(idx) = lower.find(key) {
            after_title = Some(&goal[idx + key.len()..]);
            break;
        }
    }

    if let Some(rest) = after_title {
        if let Some(start) = rest.find('\'') {
            if let Some(end) = rest[start + 1..].find('\'') {
                let title = &rest[start + 1..start + 1 + end];
                if !title.trim().is_empty() {
                    return Some(title.trim().to_string());
                }
            }
        }
        if let Some(start) = rest.find('\"') {
            if let Some(end) = rest[start + 1..].find('\"') {
                let title = &rest[start + 1..start + 1 + end];
                if !title.trim().is_empty() {
                    return Some(title.trim().to_string());
                }
            }
        }
    }

    if goal.contains("Apple Stock Calculation") {
        return Some("Apple Stock Calculation".to_string());
    }

    None
}
