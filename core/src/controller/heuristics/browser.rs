use crate::platform::{app_matches_role, current_platform, AppRole};

pub fn google_search_url(query: &str) -> String {
    let encoded = urlencoding::encode(query);
    format!("https://google.com/search?q={}", encoded)
}

pub fn google_lucky_url(query: &str) -> String {
    let encoded = urlencoding::encode(query);
    format!("https://www.google.com/search?q={}&btnI=1", encoded)
}

pub fn frontmost_browser(front_app: Option<&str>) -> Option<String> {
    let kind = current_platform().kind();
    front_app.and_then(|app| app_matches_role(kind, AppRole::Browser, app).then(|| app.to_string()))
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

#[cfg(test)]
mod tests {
    use super::frontmost_browser;
    use crate::platform::{app_role_aliases, app_role_primary_name, current_platform, AppRole};

    #[test]
    fn frontmost_browser_accepts_primary_platform_browser_name() {
        let expected = app_role_primary_name(current_platform().kind(), AppRole::Browser);
        assert_eq!(frontmost_browser(Some(expected)).as_deref(), Some(expected));
    }

    #[test]
    fn frontmost_browser_accepts_platform_browser_aliases() {
        let alias = app_role_aliases(current_platform().kind(), AppRole::Browser)
            .iter()
            .copied()
            .find(|candidate| {
                *candidate != app_role_primary_name(current_platform().kind(), AppRole::Browser)
            })
            .expect("browser alias should exist");
        assert_eq!(frontmost_browser(Some(alias)).as_deref(), Some(alias));
    }

    #[test]
    fn frontmost_browser_rejects_non_browser_apps() {
        assert_eq!(frontmost_browser(Some("Mail")), None);
    }
}
