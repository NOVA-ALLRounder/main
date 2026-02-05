use serde::{Deserialize, Serialize};

/// Action Loop Detector
/// Identifies repetitive actions to prevent agent from getting stuck.
pub struct LoopDetector;

impl LoopDetector {
    /// Detect if the same action has been repeated 3+ times (clawdbot anti-loop pattern)
    pub fn detect_action_loop(history: &[String], current_action: &str) -> bool {
        if history.len() < 2 {
            return false;
        }
        
        // Extract action type from current plan
        let current_key = Self::extract_action_key(current_action);
        
        // Check last 2 entries
        let mut match_count = 0;
        for entry in history.iter().rev().take(2) {
            if Self::extract_action_key(entry) == current_key {
                match_count += 1;
            }
        }
        
        match_count >= 2 // If last 2 are same as current, it's a loop
    }
    
    /// Extract a simplified key from action for comparison (e.g., "key:command+l" or "click:button")
    fn extract_action_key(action_str: &str) -> String {
        // Simple extraction: look for action type and key values
        if action_str.contains("\"action\":\"key\"") {
            if let Some(key_start) = action_str.find("\"key\":\"") {
                let rest = &action_str[key_start + 7..];
                if let Some(key_end) = rest.find("\"") {
                    return format!("key:{}", &rest[..key_end]);
                }
            }
        } else if action_str.contains("\"action\":\"click_visual\"") {
            return "click_visual".to_string();
        } else if action_str.contains("\"action\":\"type\"") {
            return "type".to_string();
        }
        action_str.chars().take(50).collect()
    }
}
