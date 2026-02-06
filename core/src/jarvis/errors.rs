// JarvisError - Comprehensive error types for JARVIS system
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JarvisError {
    #[error("LLM operation failed: {0}")]
    LlmError(String),

    #[error("Skill execution failed: {skill} - {reason}")]
    SkillExecutionError { skill: String, reason: String },

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type JarvisResult<T> = Result<T, JarvisError>;
