use regex::Regex;
use std::path::PathBuf;

pub fn normalize_backend_path(path: &str) -> String {
    let mut normalized = path.trim().to_string();
    if normalized.is_empty() {
        return normalized;
    }
    if let Some(idx) = normalized.find('?') {
        normalized.truncate(idx);
    }
    normalized = Regex::new(r":([A-Za-z0-9_]+)")
        .unwrap()
        .replace_all(&normalized, ":param")
        .to_string();
    normalized = trim_trailing_slash(&normalized);
    if !normalized.starts_with('/') {
        normalized = format!("/{}", normalized);
    }
    normalized
}

pub fn normalize_frontend_path(raw: &str, base_prefix: Option<&str>) -> Option<String> {
    let mut path = raw.trim().to_string();
    if path.is_empty() {
        return None;
    }

    if let Some(extracted) = extract_url_path(&path) {
        path = extracted;
    }

    if let Some(prefix) = base_prefix {
        path = path.replace("${API_BASE_URL}", prefix);
        path = path.replace("API_BASE_URL", prefix);
    }

    if let Some(idx) = path.find('?') {
        path.truncate(idx);
    }
    if let Some(idx) = path.find('#') {
        path.truncate(idx);
    }
    if path.contains("${") && !path.contains('}') {
        if let Some(idx) = path.find("${") {
            path.truncate(idx);
        }
    }

    path = Regex::new(r"\$\{[^}]+\}")
        .unwrap()
        .replace_all(&path, ":param")
        .to_string();

    if let Some(prefix) = base_prefix {
        if path.starts_with('/') && !path.starts_with(prefix) {
            path = format!("{}{}", prefix.trim_end_matches('/'), path);
        }
    }

    path = trim_trailing_slash(&path);
    if !path.starts_with('/') {
        path = format!("/{}", path);
    }
    Some(path)
}

pub fn extract_url_path(url: &str) -> Option<String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        let stripped = url.split("//").nth(1)?;
        let mut parts = stripped.splitn(2, '/');
        let _host = parts.next()?;
        let path = parts.next().unwrap_or("");
        return Some(format!("/{}", path));
    }
    if let Some(stripped) = url.strip_prefix("//") {
        let mut parts = stripped.splitn(2, '/');
        let _host = parts.next()?;
        let path = parts.next().unwrap_or("");
        return Some(format!("/{}", path));
    }
    None
}

pub fn extract_base_path(url: &str) -> Option<String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        let stripped = url.split("//").nth(1)?;
        let mut parts = stripped.splitn(2, '/');
        let _host = parts.next()?;
        let path = parts.next().unwrap_or("");
        let normalized = format!("/{}", path.trim_end_matches('/'));
        return Some(normalized);
    }
    if url.starts_with('/') {
        return Some(url.trim_end_matches('/').to_string());
    }
    None
}

pub fn trim_trailing_slash(path: &str) -> String {
    if path.len() > 1 {
        path.trim_end_matches('/').to_string()
    } else {
        path.to_string()
    }
}

pub fn paths_match(front: &str, backend: &str) -> bool {
    if front == backend {
        return true;
    }

    let front_segments = split_path(front);
    let back_segments = split_path(backend);
    if front_segments.len() != back_segments.len() {
        return false;
    }

    for (f, b) in front_segments.iter().zip(back_segments.iter()) {
        if b == ":param" {
            continue;
        }
        if f != b {
            return false;
        }
    }
    true
}

pub fn split_path(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

pub fn resolve_workdir(workdir: Option<&str>) -> PathBuf {
    workdir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}
