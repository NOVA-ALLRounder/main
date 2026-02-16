use serde::Serialize;
use std::net::TcpStream;
use std::process::Command;

#[derive(Serialize)]
pub struct Dependency {
    pub name: String,
    pub check_cmd: String,
    pub install_cmd: String,
    pub is_critical: bool,
    pub is_missing: bool, // Added field to indicate status explicitly in JSON
}

impl Dependency {
    pub fn new(name: &str, check_cmd: &str, install_cmd: &str, critical: bool) -> Self {
        Self {
            name: name.to_string(),
            check_cmd: check_cmd.to_string(),
            install_cmd: install_cmd.to_string(),
            is_critical: critical,
            is_missing: false,
        }
    }

    pub fn check(&mut self) -> bool {
        let parts: Vec<&str> = self.check_cmd.split_whitespace().collect();
        if parts.is_empty() {
            return false;
        }

        let success = Command::new(parts[0])
            .args(&parts[1..])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        self.is_missing = !success;
        success
    }
}

#[derive(Serialize)]
pub struct SystemHealth {
    pub missing_deps: Vec<Dependency>,
    pub api_port: u16,
    pub api_reachable: bool,
    pub llm_enabled: bool,
    pub gmail_credentials_set: bool,
    pub notion_ready: bool,
    pub analyzer_disabled: bool,
    pub background_analysis_disabled: bool,
    pub privacy_salt_set: bool,
    pub os: String,
    pub checked_at_utc: String,
}

impl SystemHealth {
    pub fn check_all() -> Self {
        let api_port = std::env::var("STEER_API_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(5680);
        Self::check_all_with_runtime(api_port)
    }

    pub fn check_all_with_runtime(api_port: u16) -> Self {
        let deps = if cfg!(target_os = "windows") {
            vec![Dependency::new(
                "n8n",
                "where.exe n8n",
                "npm install -g n8n",
                true,
            )]
        } else if cfg!(target_os = "macos") {
            vec![
                Dependency::new(
                    "Homebrew",
                    "which brew",
                    "/bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"",
                    true,
                ),
                Dependency::new("cliclick", "which cliclick", "brew install cliclick", false),
                Dependency::new("n8n", "which n8n", "npm install -g n8n", true),
            ]
        } else {
            vec![Dependency::new(
                "n8n",
                "which n8n",
                "npm install -g n8n",
                true,
            )]
        };

        let mut missing = Vec::new();
        for mut dep in deps {
            if !dep.check() {
                missing.push(dep);
            }
        }

        let llm_enabled = std::env::var("OPENAI_API_KEY")
            .ok()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
            || std::env::var("STEER_CLI_LLM")
                .ok()
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false);

        let gmail_credentials_set = std::path::Path::new("core/credentials.json").exists()
            || std::path::Path::new("credentials.json").exists();

        let notion_api_key_ok = std::env::var("NOTION_API_KEY")
            .ok()
            .map(|v| {
                let t = v.trim();
                !t.is_empty() && !t.eq_ignore_ascii_case("your_notion_api_key_here")
            })
            .unwrap_or(false);

        let notion_db_ok = std::env::var("NOTION_DATABASE_ID")
            .ok()
            .map(|v| {
                let t = v.trim();
                !t.is_empty() && !t.eq_ignore_ascii_case("your_database_id_here")
            })
            .unwrap_or(false);

        let notion_ready = notion_api_key_ok && notion_db_ok;

        let analyzer_disabled = std::env::var("STEER_DISABLE_ANALYZER")
            .ok()
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);

        let background_analysis_disabled = std::env::var("STEER_DISABLE_BACKGROUND_ANALYSIS")
            .ok()
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);

        let privacy_salt_set = std::env::var("PRIVACY_SALT")
            .ok()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);

        let api_reachable = TcpStream::connect(format!("127.0.0.1:{api_port}")).is_ok();

        Self {
            missing_deps: missing,
            api_port,
            api_reachable,
            llm_enabled,
            gmail_credentials_set,
            notion_ready,
            analyzer_disabled,
            background_analysis_disabled,
            privacy_salt_set,
            os: std::env::consts::OS.to_string(),
            checked_at_utc: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn print_report(&self) {
        if self.missing_deps.is_empty() {
            println!("✅ All system dependencies are satisfied.");
            return;
        }

        println!("⚠️  MISSING DEPENDENCIES DETECTED:");
        for dep in &self.missing_deps {
            println!("   - ❌ {} (Install: `{}`)", dep.name, dep.install_cmd);
        }
        println!("\nPlease install these tools for full functionality.\n");
    }
}
