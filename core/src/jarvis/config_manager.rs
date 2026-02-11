// Config Manager - TOML configuration with hot-reload
// Phase 3: Persistent configuration

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// JARVIS Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JarvisConfig {
    #[serde(default)]
    pub jarvis: JarvisSettings,

    #[serde(default)]
    pub channels: ChannelsConfig,

    #[serde(default)]
    pub skills: SkillsConfig,

    #[serde(default)]
    pub autonomous: AutonomousConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JarvisSettings {
    #[serde(default = "default_privacy_mode")]
    pub privacy_mode: String, // "basic", "full", "none"

    #[serde(default = "default_max_cost")]
    pub max_cost_per_hour: f32,

    #[serde(default)]
    pub enable_autonomous: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelsConfig {
    #[serde(default)]
    pub telegram: ChannelSettings,

    #[serde(default)]
    pub web: ChannelSettings,

    #[serde(default)]
    pub voice: ChannelSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSettings {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_min: u32,

    #[serde(default)]
    pub allowed_users: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(default)]
    pub computer_use: SkillSetting,

    #[serde(default)]
    pub email: SkillSetting,

    #[serde(default)]
    pub telegram: SkillSetting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSetting {
    #[serde(default)]
    pub enabled: bool,

    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomousConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_check_interval")]
    pub check_interval_secs: u64,

    #[serde(default)]
    pub observe_paths: Vec<String>,
}

// Default values
fn default_privacy_mode() -> String {
    "basic".to_string()
}

fn default_max_cost() -> f32 {
    1.0
}

fn default_rate_limit() -> u32 {
    30
}

fn default_check_interval() -> u64 {
    30
}

impl Default for JarvisSettings {
    fn default() -> Self {
        Self {
            privacy_mode: default_privacy_mode(),
            max_cost_per_hour: default_max_cost(),
            enable_autonomous: false,
        }
    }
}

impl Default for ChannelSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            rate_limit_per_min: default_rate_limit(),
            allowed_users: Vec::new(),
        }
    }
}

impl Default for SkillSetting {
    fn default() -> Self {
        Self {
            enabled: true,
            extra: HashMap::new(),
        }
    }
}

impl Default for ChannelsConfig {
    fn default() -> Self {
        Self {
            telegram: ChannelSettings::default(),
            web: ChannelSettings {
                enabled: true, // Web enabled by default
                ..Default::default()
            },
            voice: ChannelSettings::default(),
        }
    }
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            computer_use: SkillSetting {
                enabled: false, // Disabled by default (risky)
                ..Default::default()
            },
            email: SkillSetting::default(),
            telegram: SkillSetting::default(),
        }
    }
}

impl Default for AutonomousConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            check_interval_secs: default_check_interval(),
            observe_paths: vec![],
        }
    }
}

impl Default for JarvisConfig {
    fn default() -> Self {
        Self {
            jarvis: JarvisSettings::default(),
            channels: ChannelsConfig::default(),
            skills: SkillsConfig::default(),
            autonomous: AutonomousConfig::default(),
        }
    }
}

/// Config Manager - Loads and manages configuration
pub struct ConfigManager {
    config: Arc<RwLock<JarvisConfig>>,
    config_path: PathBuf,
}

impl ConfigManager {
    /// Load config from default path
    pub fn new() -> Result<Self> {
        let config_path = Self::default_config_path()?;
        Self::from_path(config_path)
    }

    /// Load config from specific path
    pub fn from_path(path: PathBuf) -> Result<Self> {
        let config = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            toml::from_str(&content)?
        } else {
            log::warn!("Config file not found at {:?}, using defaults", path);
            JarvisConfig::default()
        };

        // Validate config
        Self::validate(&config)?;

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            config_path: path,
        })
    }

    /// Get default config path
    fn default_config_path() -> Result<PathBuf> {
        // Windows: C:\Users\<user>\.config\steer\jarvis.toml
        // Unix: ~/.config/steer/jarvis.toml
        let config_dir = if cfg!(target_os = "windows") {
            dirs::config_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
                .join("steer")
        } else {
            dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
                .join(".config")
                .join("steer")
        };

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&config_dir)?;

        Ok(config_dir.join("jarvis.toml"))
    }

    /// Validate configuration
    fn validate(config: &JarvisConfig) -> Result<()> {
        if config.jarvis.max_cost_per_hour < 0.0 {
            anyhow::bail!("max_cost_per_hour must be positive");
        }

        if !["basic", "full", "none"].contains(&config.jarvis.privacy_mode.as_str()) {
            anyhow::bail!(
                "privacy_mode must be 'basic', 'full', or 'none', got: {}",
                config.jarvis.privacy_mode
            );
        }

        Ok(())
    }

    /// Get current config
    pub async fn get(&self) -> JarvisConfig {
        self.config.read().await.clone()
    }

    /// Update config (and save to file)
    pub async fn update(&self, config: JarvisConfig) -> Result<()> {
        // Validate
        Self::validate(&config)?;

        // Save to file
        let content = toml::to_string_pretty(&config)?;
        std::fs::write(&self.config_path, content)?;

        // Update in-memory
        *self.config.write().await = config;

        log::info!("Config updated and saved to {:?}", self.config_path);

        Ok(())
    }

    /// Reload config from file
    pub async fn reload(&self) -> Result<()> {
        if self.config_path.exists() {
            let content = std::fs::read_to_string(&self.config_path)?;
            let new_config: JarvisConfig = toml::from_str(&content)?;

            Self::validate(&new_config)?;

            *self.config.write().await = new_config;

            log::info!("Config reloaded from {:?}", self.config_path);
            Ok(())
        } else {
            anyhow::bail!("Config file not found: {:?}", self.config_path)
        }
    }

    /// Get config path
    pub fn path(&self) -> &PathBuf {
        &self.config_path
    }

    /// Start watching config file for changes (hot-reload)
    pub fn start_watch(&self) -> Result<RecommendedWatcher> {
        let config_path = self.config_path.clone();
        let config_arc = self.config.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // Only reload on modify events
                    if matches!(event.kind, EventKind::Modify(_)) {
                        log::info!("Config file changed, reloading...");

                        // Reload config
                        match std::fs::read_to_string(&config_path) {
                            Ok(content) => match toml::from_str::<JarvisConfig>(&content) {
                                Ok(new_config) => {
                                    if let Err(e) = Self::validate(&new_config) {
                                        log::error!("Config validation failed: {}", e);
                                        return;
                                    }

                                    // Update in-memory config
                                    match config_arc.try_write() {
                                        Ok(mut guard) => {
                                            *guard = new_config;
                                            log::info!("Config reloaded successfully");
                                        }
                                        Err(e) => {
                                            log::error!("Failed to acquire write lock: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to parse config: {}", e);
                                }
                            },
                            Err(e) => {
                                log::error!("Failed to read config file: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Watch error: {}", e);
                }
            }
        })?;

        // Watch the config file
        watcher.watch(&self.config_path, RecursiveMode::NonRecursive)?;

        log::info!("Started watching config file: {:?}", self.config_path);

        Ok(watcher)
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default ConfigManager")
    }
}
