use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::normalize::{
    extract_base_path, normalize_backend_path, normalize_frontend_path, paths_match,
    resolve_workdir,
};
use super::{
    BackendEndpoint, ConsistencyCheckRequest, ConsistencyCheckResult, ConsistencyIssue,
    FrontendCall,
};

pub fn run_consistency_check(req: ConsistencyCheckRequest) -> ConsistencyCheckResult {
    let workdir = resolve_workdir(req.workdir.as_deref());
    let backend = scan_backend_routes(&workdir);
    let backend_paths = backend.iter().map(|b| b.path.clone()).collect::<Vec<_>>();

    let frontend_calls = scan_frontend_calls(&workdir);

    let backend_set: HashSet<String> = backend
        .iter()
        .map(|b| normalize_backend_path(&b.path))
        .filter(|p| !p.is_empty())
        .collect();

    let mut issues = Vec::new();
    for call in &frontend_calls {
        let normalized = call.path.clone();
        if !backend_set.iter().any(|b| paths_match(&normalized, b)) {
            issues.push(ConsistencyIssue {
                path: normalized,
                reason: "Frontend call has no matching backend route".to_string(),
                source: call.source.clone(),
            });
        }
    }

    let ok = issues.is_empty();
    let summary = format!(
        "Backend routes: {}, frontend calls: {}, mismatches: {}",
        backend_paths.len(),
        frontend_calls.len(),
        issues.len()
    );

    ConsistencyCheckResult {
        ok,
        issues,
        backend_paths,
        frontend_calls,
        summary,
        template: "consistency_check".to_string(),
    }
}

pub fn scan_backend_routes(workdir: &Path) -> Vec<BackendEndpoint> {
    let mut sources = vec![workdir.join("core/src/api_server.rs")];
    let api_server_dir = workdir.join("core/src/api_server");
    if api_server_dir.exists() {
        let mut module_files = collect_files(&api_server_dir, &["rs"]);
        module_files.sort();
        module_files.retain(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name != "tests.rs")
                .unwrap_or(true)
        });
        sources.extend(module_files);
    }

    let mut endpoints = Vec::new();
    let mut seen = HashSet::new();
    for source in sources {
        for endpoint in scan_backend_routes_in_file(&source) {
            let key = (
                endpoint.path.clone(),
                endpoint.method.clone(),
                endpoint.source.clone(),
            );
            if seen.insert(key) {
                endpoints.push(endpoint);
            }
        }
    }
    endpoints
}

pub fn scan_backend_routes_in_file(path: &Path) -> Vec<BackendEndpoint> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };
    let source = path.to_string_lossy().to_string();

    let route_re = Regex::new(r#"(?s)\.route\s*\(\s*\"([^\"]+)\"\s*,\s*([a-zA-Z_:]+)"#).unwrap();
    let mut endpoints = Vec::new();

    for cap in route_re.captures_iter(&content) {
        let path = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
        if path.is_empty() {
            continue;
        }
        let method = cap.get(2).map(|m| m.as_str().to_uppercase());
        endpoints.push(BackendEndpoint {
            path,
            method,
            source: source.clone(),
        });
    }

    endpoints
}

pub fn scan_frontend_calls(workdir: &Path) -> Vec<FrontendCall> {
    let mut calls = Vec::new();
    let base_prefix = detect_api_base_prefix(workdir);

    let web_dir = workdir.join("web/src");
    if !web_dir.exists() {
        return calls;
    }

    let files = collect_files(&web_dir, &["ts", "tsx", "js", "jsx"]);

    let axios_re =
        Regex::new(r#"\b(?:api|axios)\.(get|post|put|patch|delete)\(\s*[\"'`]([^\"'`]+)[\"'`]"#)
            .unwrap();
    let fetch_re = Regex::new(r#"\bfetch\(\s*[\"'`]([^\"'`]+)[\"'`]"#).unwrap();

    for file in files {
        let content = match fs::read_to_string(&file) {
            Ok(content) => content,
            Err(_) => continue,
        };

        for cap in axios_re.captures_iter(&content) {
            let method = cap.get(1).map(|m| m.as_str().to_uppercase());
            let raw = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            if let Some(path) = normalize_frontend_path(raw, base_prefix.as_deref()) {
                calls.push(FrontendCall {
                    path,
                    method,
                    source: file.to_string_lossy().to_string(),
                });
            }
        }

        for cap in fetch_re.captures_iter(&content) {
            let raw = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            if let Some(path) = normalize_frontend_path(raw, base_prefix.as_deref()) {
                calls.push(FrontendCall {
                    path,
                    method: None,
                    source: file.to_string_lossy().to_string(),
                });
            }
        }
    }

    calls
}

pub fn detect_api_base_prefix(workdir: &Path) -> Option<String> {
    let api_file = workdir.join("web/src/lib/api.ts");
    let content = fs::read_to_string(api_file).ok()?;
    let base_re = Regex::new(r#"API_BASE_URL\s*=\s*\"([^\"]+)\""#).unwrap();
    if let Some(cap) = base_re.captures(&content) {
        return extract_base_path(cap.get(1)?.as_str());
    }

    let fallback_re = Regex::new(r#"baseURL\s*:\s*\"([^\"]+)\""#).unwrap();
    if let Some(cap) = fallback_re.captures(&content) {
        return extract_base_path(cap.get(1)?.as_str());
    }

    let normalize_default_re = Regex::new(r#"return\s+\"([^\"]+/api)\""#).unwrap();
    if let Some(cap) = normalize_default_re.captures(&content) {
        return extract_base_path(cap.get(1)?.as_str());
    }

    if content.contains("API_BASE_URL") {
        return Some("/api".to_string());
    }

    None
}

pub fn collect_files(root: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if is_ignored_dir(&path) {
                    continue;
                }
                stack.push(path);
            } else if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
                    files.push(path);
                }
            }
        }
    }
    files
}

pub fn is_ignored_dir(path: &Path) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    matches!(
        name,
        "node_modules" | "dist" | "build" | ".next" | ".git" | "target"
    )
}
