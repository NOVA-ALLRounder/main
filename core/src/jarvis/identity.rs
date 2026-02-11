// Identity System - "??롫뮉 ?袁㏓럡?硫?" (openclaw SOUL.md pattern)
//
// ~/.steer/soul.toml?癒?퐣 AI ?源껉봄????????袁⑥쨮?袁⑹뱽 嚥≪뮆諭??뺣뼄.
// ???뵬????곸몵筌?疫꿸퀡??첎誘れ몵嚥???밴쉐??뺣뼄.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// AI ?源껉봄 ?類ㅼ벥 (openclaw SOUL.md ????
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulConfig {
    pub name: String,
    pub persona: String,
    pub language: String,
    pub tone: String,
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub boundaries: Vec<String>,
    #[serde(default)]
    pub expertise: Vec<String>,
    #[serde(default)]
    pub quirks: Vec<String>,
}

impl Default for SoulConfig {
    fn default() -> Self {
        Self {
            name: "JARVIS".to_string(),
            persona: "??륁벥 ?遺????브쑴?? ?醫딅뮟??랁??醫듚??????덈뮉 AI ??쑴苑?".to_string(),
            language: "ko".to_string(),
            tone: "燁살뮄???랁?筌욊낯苑?? 鈺곕??롳쭕?.to_string(),
            values: vec![
                "??μ몛??.to_string(),
                "?類μ넇??.to_string(),
                "?袁⑥뵬??苡??鈺곕똻夷?.to_string(),
            ],
            boundaries: vec![
                "椰꾧퀣彛욑쭕?????.to_string(),
                "?븍뜇???쎈릭筌?筌뤴뫀???블????".to_string(),
            ],
            expertise: vec![
                "??곕늄?紐꾩띃??揶쏆뮆而?.to_string(),
                "??깆젟 ?온??.to_string(),
                "?類ｋ궖 野꺜??.to_string(),
            ],
            quirks: vec!["??뺤뵬??꾨립 ?醫듽돢".to_string()],
        }
    }
}

/// ??????袁⑥쨮??(openclaw USER.md ????
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub name: String,
    pub timezone: String,
    #[serde(default)]
    pub work_context: String,
    #[serde(default)]
    pub preferences: HashMap<String, String>,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            name: "User".to_string(),
            timezone: "Asia/Seoul".to_string(),
            work_context: String::new(),
            preferences: HashMap::new(),
        }
    }
}

/// soul.toml ?袁⑷퍥 ?닌듼?
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SoulFile {
    soul: SoulConfig,
    user: UserProfile,
}

/// Identity Manager - ?源껉봄????????類ｋ궖???온??
#[derive(Debug, Clone)]
pub struct IdentityManager {
    pub soul: SoulConfig,
    pub user_profile: UserProfile,
    pub(crate) config_path: PathBuf,
}

impl Default for IdentityManager {
    fn default() -> Self {
        Self {
            soul: SoulConfig::default(),
            user_profile: UserProfile::default(),
            config_path: Self::config_path(),
        }
    }
}

impl IdentityManager {
    /// ~/.steer/soul.toml?癒?퐣 嚥≪뮆諭? ??곸몵筌?疫꿸퀡??첎???밴쉐
    pub fn load_or_default() -> Result<Self> {
        let config_path = Self::config_path();

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let file: SoulFile = toml::from_str(&content).map_err(|e| {
                log::warn!("Failed to parse soul.toml: {}. Using defaults.", e);
                e
            })?;

            log::info!("Loaded identity from {:?}", config_path);
            Ok(Self {
                soul: file.soul,
                user_profile: file.user,
                config_path,
            })
        } else {
            log::info!("No soul.toml found, creating default at {:?}", config_path);
            let manager = Self {
                soul: SoulConfig::default(),
                user_profile: UserProfile::default(),
                config_path,
            };
            manager.save()?;
            Ok(manager)
        }
    }

    /// ??쇱젟 ???뵬 野껋럥以?
    fn config_path() -> PathBuf {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".steer");
        path.push("soul.toml");
        path
    }

    /// ?袁⑹삺 ??쇱젟?????뵬??????
    pub fn save(&self) -> Result<()> {
        let file = SoulFile {
            soul: self.soul.clone(),
            user: self.user_profile.clone(),
        };

        let content = toml::to_string_pretty(&file)?;

        // ?遺얠젂?醫듼봺 ??밴쉐
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&self.config_path, content)?;
        log::info!("Saved identity to {:?}", self.config_path);
        Ok(())
    }

    /// ??뽯뮞???袁⑨세?袁る뱜??Identity ?諭????밴쉐
    pub fn to_prompt_section(&self) -> String {
        let mut prompt = String::new();

        prompt.push_str("## Identity\n");
        prompt.push_str(&format!(
            "??덈뮉 {}, {}.\n",
            self.soul.name, self.soul.persona
        ));
        prompt.push_str(&format!("{}??곗쨮 ???酉鍮?\n", self.soul.tone));

        if !self.soul.expertise.is_empty() {
            prompt.push_str(&format!(
                "?袁ⓓ??브쑴鍮? {}\n",
                self.soul.expertise.join(", ")
            ));
        }

        if !self.soul.boundaries.is_empty() {
            prompt.push_str(&format!("?癒?뒅: {}\n", self.soul.boundaries.join(", ")));
        }

        if !self.soul.quirks.is_empty() {
            prompt.push_str(&format!("?諭彛? {}\n", self.soul.quirks.join(", ")));
        }

        prompt.push_str("\n## User\n");
        prompt.push_str(&format!("???????已? {}\n", self.user_profile.name));
        prompt.push_str(&format!("??볦퍢??: {}\n", self.user_profile.timezone));

        if !self.user_profile.work_context.is_empty() {
            prompt.push_str(&format!(
                "??끦??뚢뫂???쎈뱜: {}\n",
                self.user_profile.work_context
            ));
        }

        for (key, value) in &self.user_profile.preferences {
            prompt.push_str(&format!("{}: {}\n", key, value));
        }

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_soul_config() {
        let soul = SoulConfig::default();
        assert_eq!(soul.name, "JARVIS");
        assert_eq!(soul.language, "ko");
        assert!(!soul.values.is_empty());
    }

    #[test]
    fn test_default_user_profile() {
        let profile = UserProfile::default();
        assert_eq!(profile.name, "User");
        assert_eq!(profile.timezone, "Asia/Seoul");
    }

    #[test]
    fn test_toml_roundtrip() {
        let file = SoulFile {
            soul: SoulConfig::default(),
            user: UserProfile::default(),
        };

        let serialized = toml::to_string_pretty(&file).unwrap();
        let deserialized: SoulFile = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.soul.name, "JARVIS");
        assert_eq!(deserialized.user.name, "User");
    }

    #[test]
    fn test_to_prompt_section() {
        let manager = IdentityManager {
            soul: SoulConfig::default(),
            user_profile: UserProfile {
                name: "GK".to_string(),
                timezone: "Asia/Seoul".to_string(),
                work_context: "揶쏆뮆而??.to_string(),
                preferences: HashMap::new(),
            },
            config_path: PathBuf::from("/tmp/test_soul.toml"),
        };

        let prompt = manager.to_prompt_section();
        assert!(prompt.contains("JARVIS"));
        assert!(prompt.contains("GK"));
        assert!(prompt.contains("揶쏆뮆而??));
        assert!(prompt.contains("## Identity"));
        assert!(prompt.contains("## User"));
    }

    #[test]
    fn test_parse_custom_toml() {
        let toml_str = r#"
[soul]
name = "???뮞?紐껎겦"
persona = "???뮞?紐꾩뒠 AI"
language = "ko"
tone = "野꺿뫗?뉛㎗?
values = ["?類μ넇??]
boundaries = []
expertise = ["???뮞??]
quirks = []

[user]
name = "???뮞??
timezone = "UTC"
work_context = "QA"
"#;

        let file: SoulFile = toml::from_str(toml_str).unwrap();
        assert_eq!(file.soul.name, "???뮞?紐껎겦");
        assert_eq!(file.user.name, "???뮞??);
        assert_eq!(file.user.work_context, "QA");
    }
}
