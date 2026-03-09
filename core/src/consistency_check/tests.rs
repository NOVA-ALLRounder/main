use std::fs;

use super::normalize::{normalize_frontend_path, paths_match};
use super::scan::{scan_backend_routes, scan_backend_routes_in_file};

#[test]
fn test_paths_match_param() {
    assert!(paths_match("/api/routines/123", "/api/routines/:param"));
    assert!(!paths_match(
        "/api/routines/123/abc",
        "/api/routines/:param"
    ));
}

#[test]
fn test_normalize_frontend_with_base() {
    let path = normalize_frontend_path("/status", Some("/api")).unwrap();
    assert_eq!(path, "/api/status");
}

#[test]
fn test_normalize_frontend_with_api_base_url_template() {
    let path = normalize_frontend_path("${API_BASE_URL}/chat", Some("/api")).unwrap();
    assert_eq!(path, "/api/chat");
}

#[test]
fn test_normalize_frontend_truncates_unclosed_query_template_suffix() {
    let path = normalize_frontend_path("/workflow/provision-ops${qs ? ", Some("/api")).unwrap();
    assert_eq!(path, "/api/workflow/provision-ops");
}

#[test]
fn test_scan_backend_routes_supports_multiline_route_declarations() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let file_path = temp_dir.path().join("api_server.rs");
    fs::write(
        &file_path,
        r#"
            let app = Router::new()
                .route(
                    "/api/launch/eval-candidates",
                    get(get_launch_eval_candidates_handler),
                )
                .route(
                    "/api/routines/:id",
                    axum::routing::patch(toggle_routine_handler),
                );
            "#,
    )
    .expect("write api file");

    let routes = scan_backend_routes_in_file(&file_path);
    let paths = routes
        .into_iter()
        .map(|route| route.path)
        .collect::<Vec<_>>();
    assert!(paths.contains(&"/api/launch/eval-candidates".to_string()));
    assert!(paths.contains(&"/api/routines/:id".to_string()));
}

#[test]
fn test_scan_backend_routes_reads_api_server_modules() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let root = temp_dir.path();
    let api_server_dir = root.join("core/src/api_server");
    fs::create_dir_all(&api_server_dir).expect("create api_server dir");
    fs::write(root.join("core/src/api_server.rs"), "// root shim").expect("write root shim");
    fs::write(
        api_server_dir.join("server.rs"),
        r#"
            let app = Router::new()
                .route("/api/chat", post(handle_chat))
                .route("/api/release/readiness", get(get_latest_release_readiness_handler));
            "#,
    )
    .expect("write server routes");
    fs::write(
        api_server_dir.join("tests.rs"),
        r#"
            let app = Router::new().route("/api/test-only", get(test_handler));
            "#,
    )
    .expect("write test routes");

    let routes = scan_backend_routes(root);
    let paths = routes
        .into_iter()
        .map(|route| route.path)
        .collect::<Vec<_>>();
    assert!(paths.contains(&"/api/chat".to_string()));
    assert!(paths.contains(&"/api/release/readiness".to_string()));
    assert!(!paths.contains(&"/api/test-only".to_string()));
}
