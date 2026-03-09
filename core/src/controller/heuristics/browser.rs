pub fn google_search_url(query: &str) -> String {
    let encoded = urlencoding::encode(query);
    format!("https://google.com/search?q={}", encoded)
}

pub fn google_lucky_url(query: &str) -> String {
    let encoded = urlencoding::encode(query);
    format!("https://www.google.com/search?q={}&btnI=1", encoded)
}

pub fn frontmost_browser(front_app: Option<&str>) -> Option<&'static str> {
    match front_app {
        Some(app) if app.eq_ignore_ascii_case("Safari") => Some("Safari"),
        Some(app) if app.eq_ignore_ascii_case("Google Chrome") => Some("Google Chrome"),
        _ => None,
    }
}

pub fn is_google_search_goal(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    lower.contains("google") || lower.contains("검색")
}

pub fn wants_first_result(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    lower.contains("first result")
        || lower.contains("첫 번째 결과")
        || lower.contains("첫번째 결과")
        || (lower.contains("첫") && lower.contains("결과"))
}

pub fn prefer_lucky_only(goal: &str) -> bool {
    wants_first_result(goal) && is_google_search_goal(goal)
}

pub fn extract_query_param(url: &str, key: &str) -> Option<String> {
    let qs = url.split_once('?')?.1;
    for pair in qs.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        if k != key {
            continue;
        }
        let v = it.next().unwrap_or("");
        if let Ok(decoded) = urlencoding::decode(v) {
            let out = decoded.into_owned();
            if !out.is_empty() {
                return Some(out);
            }
        }
    }
    None
}

pub fn extract_google_redirect_target(url: &str) -> Option<String> {
    if !(url.contains("google.com/url?")
        || url.contains("google.co.kr/url?")
        || url.contains("google.com/url?q="))
    {
        return None;
    }
    extract_query_param(url, "url")
        .or_else(|| extract_query_param(url, "q"))
        .or_else(|| extract_query_param(url, "target"))
}

pub fn is_redirect_alert(title: &str, url: &str) -> bool {
    let t = title.to_lowercase();
    let u = url.to_lowercase();
    t.contains("리디렉션")
        || t.contains("redirect")
        || u.contains("google.com/url?")
        || u.contains("google.co.kr/url?")
}
