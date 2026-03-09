use super::super::*;

impl N8nApi {
    fn local_db_candidates() -> Vec<PathBuf> {
        if let Ok(custom) = std::env::var("STEER_N8N_DB_PATH") {
            let trimmed = custom.trim();
            if !trimmed.is_empty() {
                return vec![PathBuf::from(trimmed)];
            }
        }

        let home = match std::env::var("HOME") {
            Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
            _ => return Vec::new(),
        };

        vec![
            home.join(".steer").join("n8n").join("database.sqlite"),
            home.join(".n8n").join("database.sqlite"),
        ]
    }

    fn local_latest_api_key_from_db() -> Option<String> {
        for db_path in Self::local_db_candidates() {
            if !db_path.exists() {
                continue;
            }

            let conn = match Connection::open(&db_path) {
                Ok(conn) => conn,
                Err(_) => continue,
            };
            let key: String = match conn.query_row(
                "SELECT apiKey FROM user_api_keys ORDER BY createdAt DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            ) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let trimmed = key.trim();
            if trimmed.is_empty() {
                continue;
            }
            return Some(trimmed.to_string());
        }

        None
    }

    fn n8n_cli_binary_available() -> bool {
        Command::new("n8n")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    pub(crate) fn resolve_cli_invocation(&self) -> Result<(String, Vec<String>)> {
        if Self::n8n_cli_binary_available() {
            return Ok(("n8n".to_string(), Vec::new()));
        }

        let allow_npx_cli = parse_bool_env_with_default("STEER_N8N_ALLOW_NPX_CLI", false);
        if allow_npx_cli || matches!(self.runtime_mode(), N8nRuntime::Npx) {
            let test_context = n8n_test_context();
            if allow_npx_cli
                && !test_context
                && !parse_bool_env_with_default("STEER_N8N_ALLOW_NPX_CLI_NON_TEST", false)
                && !matches!(self.runtime_mode(), N8nRuntime::Npx)
            {
                return Err(anyhow::anyhow!(
                    "npx CLI fallback is test/CI-only by default. \
Set STEER_N8N_ALLOW_NPX_CLI_NON_TEST=1 to allow outside test mode."
                ));
            }
            if !self.local_target()
                && !parse_bool_env_with_default("STEER_N8N_ALLOW_NPX_CLI_REMOTE", false)
            {
                return Err(anyhow::anyhow!(
                    "npx CLI fallback is blocked for remote N8N_API_URL (set STEER_N8N_ALLOW_NPX_CLI_REMOTE=1 to override)."
                ));
            }
            if !parse_bool_env_with_default("STEER_N8N_ENABLE_NPX_RUNTIME", false) && !allow_npx_cli
            {
                return Err(anyhow::anyhow!(
                    "n8n CLI binary is missing and npx CLI fallback is disabled. \
Set STEER_N8N_ALLOW_NPX_CLI=1 (or enable npx runtime explicitly)."
                ));
            }
            return Ok(("npx".to_string(), vec!["-y".to_string(), "n8n".to_string()]));
        }

        Err(anyhow::anyhow!(
            "n8n CLI binary not found in PATH. Install n8n or set STEER_N8N_ALLOW_NPX_CLI=1"
        ))
    }

    fn build_http_client() -> Client {
        let prefer_no_proxy =
            cfg!(test) || parse_bool_env_with_default("STEER_HTTP_NO_SYSTEM_PROXY", false);
        if prefer_no_proxy {
            if let Ok(client) = Client::builder().no_proxy().build() {
                return client;
            }
        }

        if let Ok(client) = std::panic::catch_unwind(Client::new) {
            return client;
        }

        eprintln!("⚠️ reqwest default client init panicked; falling back to no-proxy client");
        Client::builder()
            .no_proxy()
            .build()
            .unwrap_or_else(|_| Client::new())
    }

    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client: Self::build_http_client(),
        }
    }

    pub fn from_env() -> anyhow::Result<Self> {
        let base_url = std::env::var("STEER_N8N_API_URL")
            .or_else(|_| std::env::var("N8N_API_URL"))
            .unwrap_or_else(|_| "http://localhost:5678/api/v1".to_string());
        // Allow missing key only when CLI fallback is explicitly enabled.
        let mut api_key = std::env::var("STEER_N8N_API_KEY")
            .or_else(|_| std::env::var("N8N_API_KEY"))
            .unwrap_or_default();
        let prefer_db_key = parse_bool_env_with_default("STEER_N8N_PREFER_LOCAL_DB_KEY", true);
        let local_target = base_url.contains("localhost") || base_url.contains("127.0.0.1");
        if prefer_db_key && local_target {
            if let Some(db_key) = Self::local_latest_api_key_from_db() {
                if api_key.trim() != db_key {
                    api_key = db_key;
                }
            }
        }
        Ok(Self::new(&base_url, &api_key))
    }

    pub(crate) fn runtime_mode(&self) -> N8nRuntime {
        N8nRuntime::from_env()
    }

    pub(crate) fn auto_start_enabled(&self, runtime: N8nRuntime) -> bool {
        let default = matches!(runtime, N8nRuntime::Docker);
        parse_bool_env_with_default("STEER_N8N_AUTO_START", default)
    }

    pub(crate) fn cli_fallback_enabled(&self, runtime: N8nRuntime) -> bool {
        let default = matches!(runtime, N8nRuntime::Npx);
        parse_bool_env_with_default("STEER_N8N_ALLOW_CLI_FALLBACK", default)
    }

    pub(crate) fn local_target(&self) -> bool {
        self.base_url.contains("localhost")
            || self.base_url.contains("127.0.0.1")
            || self.base_url.contains("0.0.0.0")
            || self.base_url.contains("::1")
    }

    pub(crate) fn reserve_ephemeral_local_port() -> Option<u16> {
        std::net::TcpListener::bind(("127.0.0.1", 0))
            .ok()
            .and_then(|listener| listener.local_addr().ok().map(|addr| addr.port()))
    }

    pub(super) fn health_urls(&self) -> (String, String) {
        let root_url = self
            .base_url
            .replace("localhost", "127.0.0.1")
            .replace("/api/v1", "/");
        let root_trimmed = root_url.trim_end_matches('/');
        let healthz = format!("{}/healthz", root_trimmed);
        (healthz, format!("{}/", root_trimmed))
    }

    fn http_retry_attempts(&self) -> u32 {
        parse_u32_env_with_default("STEER_N8N_HTTP_RETRY_ATTEMPTS", 4, 1, 8)
    }

    fn http_retry_min_backoff_ms(&self) -> u64 {
        parse_u64_env_with_default("STEER_N8N_HTTP_RETRY_MIN_BACKOFF_MS", 400, 100, 60_000)
    }

    fn http_retry_max_backoff_ms(&self) -> u64 {
        parse_u64_env_with_default("STEER_N8N_HTTP_RETRY_MAX_BACKOFF_MS", 10_000, 500, 120_000)
    }

    fn http_retry_jitter(&self) -> f64 {
        parse_f64_env_with_default("STEER_N8N_HTTP_RETRY_JITTER", 0.1, 0.0, 0.5)
    }

    fn http_request_timeout_ms(&self) -> u64 {
        parse_u64_env_with_default("STEER_N8N_HTTP_REQUEST_TIMEOUT_MS", 12_000, 1_000, 120_000)
    }

    pub(crate) async fn send_with_retry<F>(&self, label: &str, mut build: F) -> Result<Response>
    where
        F: FnMut() -> reqwest::RequestBuilder,
    {
        let policy = crate::retry_policy::RetryPolicy::new(
            self.http_retry_attempts(),
            self.http_retry_min_backoff_ms(),
            self.http_retry_max_backoff_ms(),
            self.http_retry_jitter(),
        );
        let request_timeout = Duration::from_millis(self.http_request_timeout_ms());

        for attempt in 1..=policy.attempts {
            match build().timeout(request_timeout).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        return Ok(resp);
                    }
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    let retryable = crate::retry_policy::retryable_http_status(status);
                    if retryable && attempt < policy.attempts {
                        let retry_after = retry_after_ms_from_status_and_body(status, &body);
                        let delay = crate::retry_policy::compute_backoff_delay_ms(
                            policy,
                            attempt - 1,
                            retry_after,
                            label,
                        );
                        crate::diagnostic_events::emit(
                            "n8n.http.retry",
                            json!({
                                "label": label,
                                "attempt": attempt,
                                "status": status.as_u16(),
                                "delay_ms": delay,
                                "retry_after_ms": retry_after
                            }),
                        );
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        continue;
                    }
                    return Err(anyhow::anyhow!(
                        "{} failed (status={}): {}",
                        label,
                        status,
                        body
                    ));
                }
                Err(err) => {
                    let retryable = err.is_timeout() || err.is_connect() || err.is_request();
                    if retryable && attempt < policy.attempts {
                        let delay = crate::retry_policy::compute_backoff_delay_ms(
                            policy,
                            attempt - 1,
                            None,
                            label,
                        );
                        crate::diagnostic_events::emit(
                            "n8n.http.retry",
                            json!({
                                "label": label,
                                "attempt": attempt,
                                "error": err.to_string(),
                                "delay_ms": delay
                            }),
                        );
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        continue;
                    }
                    return Err(anyhow::anyhow!("{} request failed: {}", label, err));
                }
            }
        }

        Err(anyhow::anyhow!(
            "{} failed after {} attempt(s)",
            label,
            policy.attempts
        ))
    }
}
