// Privacy mode configuration

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrivacyMode {
    /// Basic mode: Only app names and window titles
    Basic,
    /// Full mode: + Screenshots every 30 seconds
    Full,
    /// Dev mode: + Keyboard/mouse events (full logging)
    Dev,
}

impl Default for PrivacyMode {
    fn default() -> Self {
        Self::Basic
    }
}

impl PrivacyMode {
    pub fn allows_screenshots(&self) -> bool {
        matches!(self, Self::Full | Self::Dev)
    }

    pub fn allows_input_logging(&self) -> bool {
        matches!(self, Self::Dev)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Full => "full",
            Self::Dev => "dev",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "basic" => Some(Self::Basic),
            "full" => Some(Self::Full),
            "dev" => Some(Self::Dev),
            _ => None,
        }
    }
}
