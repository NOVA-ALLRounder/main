pub(super) async fn ingest_events(
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    // 1. Normalize
    let events: Vec<crate::schema::EventEnvelope> = if let Some(arr) = payload.as_array() {
        arr.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect()
    } else if let Ok(single) = serde_json::from_value(payload.clone()) {
        vec![single]
    } else {
        return Json(serde_json::json!({ "error": "Invalid Event Format", "count": 0 }));
    };

    let count = events.len();
    let use_dcp = collector_bridge::is_dcp_mode();
    let mut success = 0;
    if use_dcp {
        match collector_bridge::send_events(&events).await {
            Ok(sent) => success = sent,
            Err(e) => {
                eprintln!("Ingest Error (DCP): {}", e);
            }
        }
    } else {
        // 2. Process & Insert
        // In a real high-perf scenario, we would push to a channel (EventBus).
        // For now, direct DB insert is fast enough for direct migration.
        
        // Initialize Privacy Guard (Salt should come from env in prod)
        let salt = std::env::var("PRIVACY_SALT").unwrap_or_else(|_| "default_salt".to_string());
        let guard = crate::privacy::PrivacyGuard::new(salt);
        
        let mut masked = Vec::with_capacity(events.len());
        for event in events {
            // [Privacy] Apply masking
            if let Some(masked_event) = guard.apply(event) {
                masked.push(masked_event);
            } else {
                // Dropped by privacy rules (e.g. deny list)
                println!("Event dropped by PrivacyGuard");
            }
        }

        match db::insert_events_v2_batch(&masked) {
            Ok(inserted) => success += inserted,
            Err(e) => eprintln!("Ingest Batch Error: {}", e),
        }
    }

    Json(serde_json::json!({
        "status": if use_dcp { "forwarded" } else { "queued" },
        "received": count,
        "processed": success
    }))
}



