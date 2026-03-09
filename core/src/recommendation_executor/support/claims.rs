use super::super::*;

pub(in crate::recommendation_executor) fn provisioning_claim_ttl_millis() -> i64 {
    std::env::var("STEER_PROVISIONING_CLAIM_TTL_SECONDS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .map(|seconds| seconds.clamp(1, 86_400) * 1_000)
        .unwrap_or(10 * 60 * 1_000)
}

pub(in crate::recommendation_executor) fn parse_claim_timestamp_millis(token: &str) -> Option<i64> {
    if !token.starts_with("provisioning:") {
        return None;
    }
    token.rsplit(':').next()?.parse::<i64>().ok()
}

pub(in crate::recommendation_executor) fn is_stale_provisioning_claim(token: &str) -> bool {
    let ts = match parse_claim_timestamp_millis(token) {
        Some(v) => v,
        None => return false,
    };
    let now = chrono::Utc::now().timestamp_millis();
    now.saturating_sub(ts) > provisioning_claim_ttl_millis()
}

pub(in crate::recommendation_executor) fn make_claim_token(id: i64) -> String {
    format!(
        "provisioning:{}:{}",
        id,
        chrono::Utc::now().timestamp_millis()
    )
}

pub(in crate::recommendation_executor) fn claim_or_get_existing(
    id: i64,
    claim_token: &str,
) -> Result<Option<String>> {
    for _ in 0..2 {
        match db::claim_recommendation_provisioning(id, claim_token)? {
            Some(existing) => {
                let trimmed = existing.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed == claim_token {
                    return Ok(None);
                }
                if trimmed.starts_with("provisioning:") {
                    if is_stale_provisioning_claim(trimmed) {
                        let _ = db::release_recommendation_provisioning_claim(id, trimmed);
                        continue;
                    }
                    return Err(anyhow!(
                        "recommendation {} is already being provisioned ({})",
                        id,
                        trimmed
                    ));
                }
                return Ok(Some(trimmed.to_string()));
            }
            None => return Ok(None),
        }
    }
    Err(anyhow!(
        "failed to acquire provisioning claim for recommendation {}",
        id
    ))
}
