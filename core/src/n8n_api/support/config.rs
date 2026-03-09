use reqwest::StatusCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum N8nRuntime {
    Docker,
    Npx,
    Manual,
}

impl N8nRuntime {
    pub(crate) fn from_env() -> Self {
        let raw = std::env::var("STEER_N8N_RUNTIME")
            .unwrap_or_else(|_| "manual".to_string())
            .trim()
            .to_lowercase();
        let test_context = parse_bool_env_with_default("STEER_TEST_MODE", false)
            || parse_bool_env_with_default("CI", false);
        match raw.as_str() {
            "npx" => {
                if !parse_bool_env_with_default("STEER_N8N_ENABLE_NPX_RUNTIME", false) {
                    eprintln!(
                        "⚠️ STEER_N8N_RUNTIME=npx ignored: set STEER_N8N_ENABLE_NPX_RUNTIME=1 to opt in."
                    );
                    return Self::Manual;
                }
                if !test_context
                    && !parse_bool_env_with_default("STEER_N8N_ALLOW_NPX_NON_TEST", false)
                {
                    eprintln!(
                        "⚠️ STEER_N8N_RUNTIME=npx ignored outside test mode. \
Set STEER_N8N_ALLOW_NPX_NON_TEST=1 to force npx runtime."
                    );
                    return Self::Manual;
                }
                Self::Npx
            }
            "manual" | "none" => Self::Manual,
            _ => Self::Docker,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Npx => "npx",
            Self::Manual => "manual",
        }
    }
}

pub(crate) fn parse_bool_env_with_default(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

pub(crate) fn parse_u32_env_with_default(key: &str, default: u32, min: u32, max: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default.clamp(min, max))
}

pub(crate) fn parse_u64_env_with_default(key: &str, default: u64, min: u64, max: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default.clamp(min, max))
}

pub(crate) fn parse_f64_env_with_default(key: &str, default: f64, min: f64, max: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default.clamp(min, max))
}

pub(crate) fn retry_after_ms_from_status_and_body(status: StatusCode, body: &str) -> Option<u64> {
    if status != StatusCode::TOO_MANY_REQUESTS && !status.is_server_error() {
        return None;
    }
    crate::retry_policy::parse_retry_after_ms(body)
}

pub(crate) fn n8n_test_context() -> bool {
    parse_bool_env_with_default("STEER_TEST_MODE", false)
        || parse_bool_env_with_default("CI", false)
}
