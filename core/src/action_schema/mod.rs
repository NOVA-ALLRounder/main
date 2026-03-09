use serde_json::Value;

mod normalize;

#[derive(Debug, Clone)]
pub struct ActionValidation {
    pub normalized: Value,
    pub error: Option<String>,
}

pub use normalize::normalize_action;

#[cfg(test)]
mod tests;
