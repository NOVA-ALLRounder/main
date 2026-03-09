use super::support::{floor_to_five_minute_bucket, infer_action_type, request_is_authorized};
use super::*;
use axum::http::{HeaderMap, HeaderValue};
use chrono::{TimeZone, Timelike, Utc};

#[test]
fn floors_to_five_minute_bucket() {
    let ts = Utc.with_ymd_and_hms(2026, 2, 11, 13, 17, 44).unwrap();
    let bucket = floor_to_five_minute_bucket(ts);
    assert_eq!(bucket.minute(), 15);
    assert_eq!(bucket.second(), 0);
}

#[test]
fn infers_action_types() {
    assert_eq!(infer_action_type("Button"), "click");
    assert_eq!(infer_action_type("Edit"), "type");
    assert_eq!(infer_action_type("Unknown"), "interact");
}

#[test]
fn request_auth_passes_without_configured_token() {
    let state = AppState {
        started_at: Instant::now(),
        stats: Arc::new(Mutex::new(IngestStats::default())),
        guard: Arc::new(PrivacyGuard::new("test-salt".to_string())),
        ingest_token: None,
    };
    let headers = HeaderMap::new();
    assert!(request_is_authorized(&state, &headers));
}

#[test]
fn request_auth_accepts_matching_header_token() {
    let state = AppState {
        started_at: Instant::now(),
        stats: Arc::new(Mutex::new(IngestStats::default())),
        guard: Arc::new(PrivacyGuard::new("test-salt".to_string())),
        ingest_token: Some("secret".to_string()),
    };
    let mut headers = HeaderMap::new();
    headers.insert("x-collector-token", HeaderValue::from_static("secret"));
    assert!(request_is_authorized(&state, &headers));
}

#[test]
fn request_auth_rejects_wrong_token() {
    let state = AppState {
        started_at: Instant::now(),
        stats: Arc::new(Mutex::new(IngestStats::default())),
        guard: Arc::new(PrivacyGuard::new("test-salt".to_string())),
        ingest_token: Some("secret".to_string()),
    };
    let mut headers = HeaderMap::new();
    headers.insert("x-collector-token", HeaderValue::from_static("wrong"));
    assert!(!request_is_authorized(&state, &headers));
}
