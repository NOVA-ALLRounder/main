use super::super::*;

impl N8nApi {
    async fn is_server_reachable(&self, healthz: &str, root: &str) -> bool {
        let timeout = std::time::Duration::from_secs(2);
        let ok_health = self
            .client
            .get(healthz)
            .timeout(timeout)
            .send()
            .await
            .map(|resp| resp.status().is_success())
            .unwrap_or(false);
        if ok_health {
            return true;
        }

        self.client
            .get(root)
            .timeout(timeout)
            .send()
            .await
            .map(|resp| resp.status().is_success())
            .unwrap_or(false)
    }

    fn resolve_compose_file() -> Option<PathBuf> {
        if let Ok(raw) = std::env::var("STEER_N8N_COMPOSE_FILE") {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed));
            }
        }

        let cwd = std::env::current_dir().ok()?;
        let candidates = [
            cwd.join("docker-compose.yml"),
            cwd.join("../docker-compose.yml"),
        ];
        candidates.into_iter().find(|p| p.is_file())
    }

    fn run_docker_compose(compose_file: &Path, args: &[&str]) -> Result<()> {
        let compose_file_str = compose_file.display().to_string();
        let run_primary = std::process::Command::new("docker")
            .arg("compose")
            .arg("-f")
            .arg(&compose_file_str)
            .args(args)
            .output();

        match run_primary {
            Ok(out) if out.status.success() => return Ok(()),
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let detail = if !stderr.is_empty() { stderr } else { stdout };
                eprintln!(
                    "⚠️ docker compose failed, trying docker-compose fallback: {}",
                    detail
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(anyhow::anyhow!("failed to run docker compose: {}", e)),
        }

        let legacy = std::process::Command::new("docker-compose")
            .arg("-f")
            .arg(&compose_file_str)
            .args(args)
            .output();
        match legacy {
            Ok(out) if out.status.success() => Ok(()),
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let detail = if !stderr.is_empty() { stderr } else { stdout };
                Err(anyhow::anyhow!("docker-compose failed: {}", detail))
            }
            Err(e) => Err(anyhow::anyhow!(
                "docker compose unavailable (tried docker compose + docker-compose): {}",
                e
            )),
        }
    }

    fn start_with_docker(&self) -> Result<()> {
        let compose_file = Self::resolve_compose_file().ok_or_else(|| {
            anyhow::anyhow!(
                "docker-compose.yml not found. Place it at repo root or set STEER_N8N_COMPOSE_FILE"
            )
        })?;
        println!(
            "🐳 Starting n8n via Docker Compose (runtime=docker, file={})...",
            compose_file.display()
        );
        Self::run_docker_compose(&compose_file, &["up", "-d", "n8n"])
    }

    fn start_with_npx(&self) -> Result<()> {
        if !parse_bool_env_with_default("STEER_N8N_ENABLE_NPX_RUNTIME", false) {
            return Err(anyhow::anyhow!(
                "npx runtime is disabled by default. Set STEER_N8N_ENABLE_NPX_RUNTIME=1 to enable."
            ));
        }
        let test_context = n8n_test_context();
        let requested_tunnel = crate::env_flag("STEER_N8N_USE_TUNNEL");
        if requested_tunnel
            && !test_context
            && !parse_bool_env_with_default("STEER_N8N_ALLOW_NPX_TUNNEL_NON_TEST", false)
        {
            crate::diagnostic_events::emit(
                "n8n.runtime.npx.blocked",
                json!({
                    "reason": "tunnel_non_test_blocked",
                    "test_context": test_context
                }),
            );
            return Err(anyhow::anyhow!(
                "npx --tunnel is test/CI-only by default. \
Set STEER_N8N_ALLOW_NPX_TUNNEL_NON_TEST=1 to override."
            ));
        }
        println!("⚠️  Starting n8n with npx fallback runtime...");
        let mut args = vec!["-y", "n8n", "start"];
        if requested_tunnel {
            args.push("--tunnel");
        }
        crate::diagnostic_events::emit(
            "n8n.runtime.npx.start",
            json!({
                "tunnel": requested_tunnel,
                "test_context": test_context
            }),
        );

        Command::new("npx")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to auto-start n8n with npx: {}", e))?;
        Ok(())
    }

    fn start_runtime(&self, runtime: N8nRuntime) -> Result<()> {
        match runtime {
            N8nRuntime::Docker => self.start_with_docker(),
            N8nRuntime::Npx => self.start_with_npx(),
            N8nRuntime::Manual => Err(anyhow::anyhow!(
                "runtime=manual: start n8n yourself and set N8N_API_URL/N8N_API_KEY"
            )),
        }
    }

    pub async fn restart_server(&self) -> Result<()> {
        if crate::env_flag("STEER_N8N_MOCK") {
            println!("🧪 STEER_N8N_MOCK=1: skipping n8n restart");
            return Ok(());
        }

        let runtime = self.runtime_mode();
        if !self.local_target() && !matches!(runtime, N8nRuntime::Manual) {
            return Err(anyhow::anyhow!(
                "runtime={} cannot restart remote n8n target ({})",
                runtime.as_str(),
                self.base_url
            ));
        }

        match runtime {
            N8nRuntime::Docker => {
                let compose_file = Self::resolve_compose_file().ok_or_else(|| {
                    anyhow::anyhow!(
                        "docker-compose.yml not found. Place it at repo root or set STEER_N8N_COMPOSE_FILE"
                    )
                })?;
                println!(
                    "🐳 Restarting n8n via Docker Compose (file={})...",
                    compose_file.display()
                );
                if let Err(restart_err) =
                    Self::run_docker_compose(&compose_file, &["restart", "n8n"])
                {
                    eprintln!(
                        "⚠️ Docker restart failed ({}). Trying `up -d n8n`...",
                        restart_err
                    );
                    Self::run_docker_compose(&compose_file, &["up", "-d", "n8n"])?;
                }
            }
            N8nRuntime::Npx => {
                let _ = std::process::Command::new("pkill")
                    .arg("-f")
                    .arg("n8n")
                    .output();
                self.start_with_npx()?;
            }
            N8nRuntime::Manual => {
                return Err(anyhow::anyhow!(
                    "runtime=manual: cannot restart automatically"
                ));
            }
        }

        let (healthz, root) = self.health_urls();
        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if self.is_server_reachable(&healthz, &root).await {
                println!("✅ n8n restart completed.");
                return Ok(());
            }
        }
        Err(anyhow::anyhow!("Timed out waiting for n8n after restart"))
    }

    /// Check if n8n is running, and start it if not
    pub async fn ensure_server_running(&self) -> Result<()> {
        if crate::env_flag("STEER_N8N_MOCK") {
            println!("🧪 STEER_N8N_MOCK=1: skipping n8n health/start checks");
            return Ok(());
        }

        let runtime = self.runtime_mode();
        let (healthz, root) = self.health_urls();
        println!(
            "🔎 Checking n8n health (runtime={}, healthz={})...",
            runtime.as_str(),
            healthz
        );

        if self.is_server_reachable(&healthz, &root).await {
            println!("✅ n8n server is running.");
            return Ok(());
        }

        if !self.auto_start_enabled(runtime) {
            return Err(anyhow::anyhow!(
                "n8n server is not reachable at {}. Enable auto-start with STEER_N8N_AUTO_START=1 or run n8n manually.",
                healthz
            ));
        }

        if !self.local_target() && !matches!(runtime, N8nRuntime::Manual) {
            return Err(anyhow::anyhow!(
                "runtime={} cannot auto-start remote n8n target ({})",
                runtime.as_str(),
                self.base_url
            ));
        }

        println!("⚠️  n8n server NOT found. Starting automatically...");
        self.start_runtime(runtime)?;

        println!("⏳ Waiting for n8n to initialize (this may take 60s)...");
        for i in 0..30 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if self.is_server_reachable(&healthz, &root).await {
                println!("🚀 n8n server started successfully!");
                if !self.api_key.is_empty() && self.api_key != "placeholder" {
                    self.verify_auth().await?;
                }
                return Ok(());
            }
            if i % 5 == 0 {
                println!("... still waiting ({}/60s)", i * 2);
            }
        }

        Err(anyhow::anyhow!("Timed out waiting for n8n to start."))
    }

    /// Helper: Verify API Key works
    pub async fn verify_auth(&self) -> Result<()> {
        println!("🔐 Verifying n8n API Key...");
        // Try a lightweight authenticated call
        let url = format!("{}/workflows?limit=1", self.base_url);

        let resp = self
            .send_with_retry("n8n verify_auth", || {
                self.client
                    .get(&url)
                    .header("X-N8N-API-KEY", &self.api_key)
                    .timeout(std::time::Duration::from_secs(3))
            })
            .await?;

        if resp.status().is_success() {
            println!("✅ API Key is valid.");
            Ok(())
        } else if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            Err(anyhow::anyhow!(
                "❌ n8n API Key is INVALID ({}). Check core/.env or secrets.",
                resp.status()
            ))
        } else {
            Err(anyhow::anyhow!(
                "❌ n8n auth verification failed with status {}",
                resp.status()
            ))
        }
    }

    /// List available credentials
    pub async fn list_credentials(&self) -> Result<Vec<Credential>> {
        if crate::env_flag("STEER_N8N_MOCK") {
            return Ok(Vec::new());
        }

        let url = format!("{}/credentials", self.base_url);
        let resp = self
            .send_with_retry("n8n list_credentials", || {
                self.client.get(&url).header("X-N8N-API-KEY", &self.api_key)
            })
            .await?;

        if !resp.status().is_success() {
            return Ok(Vec::new()); // Return empty if failed (e.g. auth error)
        }

        let json: Value = resp.json().await?;
        let data = json["data"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid credentials response"))?;

        // n8n API structure differs by version, trying to extract minimal info
        let credentials = data
            .iter()
            .map(|c| Credential {
                id: c["id"].as_str().unwrap_or("").to_string(),
                name: c["name"].as_str().unwrap_or("").to_string(),
                type_name: c["type"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(credentials)
    }
}
