pub enum SafetyLevel {
    Safe,
    Warning,
    Critical,
}

pub struct CommandClassifier;

impl CommandClassifier {
    pub fn classify(cmd: &str) -> SafetyLevel {
        let cmd = cmd.trim();
        // Normalize: Collapse multiple spaces to one used for pattern matching
        let normalized: String = cmd.split_whitespace().collect::<Vec<_>>().join(" ");
        let check_target = if normalized.is_empty() { cmd } else { &normalized };
        
        // 1. Critical Commands (High Risk)
        // Fork bombs, filesystem wipe, root escalation
        if check_target.contains("sudo") || 
           check_target.contains("rm -rf") || 
           check_target.contains("dd if=") || 
           check_target.contains("mkfs") || 
           check_target.contains(":(){ :|:& };:") {
            return SafetyLevel::Critical;
        }

        // 2. Warning Commands (Medium Risk)
        // File deletion, modification, network requests
        if check_target.starts_with("rm") || 
           check_target.starts_with("mv") || 
           check_target.starts_with("curl") || 
           check_target.starts_with("wget") || 
           check_target.starts_with("chmod") ||
           check_target.starts_with("chown") ||
           check_target.contains(">") { // Redirection could overwrite files
            return SafetyLevel::Warning;
        }

        // 3. Safe Commands (Low Risk)
        // Read-only or harmless operations
        SafetyLevel::Safe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_critical_commands_detected() {
        let cases = vec![
            "sudo rm -rf /",
            "sudo apt install malware",
            "rm -rf /var",
            "dd if=/dev/zero of=/dev/sda",
            "mkfs.ext4 /dev/sda1",
            ":(){ :|:& };:",  // Fork bomb
        ];

        for cmd in cases {
            match CommandClassifier::classify(cmd) {
                SafetyLevel::Critical => {},  // Expected
                _ => panic!("Command '{}' should be classified as Critical", cmd),
            }
        }
    }

    #[test]
    fn test_warning_commands_detected() {
        let cases = vec![
            "rm file.txt",
            "mv old.txt new.txt",
            "curl https://example.com",
            "wget https://example.com/file.zip",
            "chmod 777 file.txt",
            "chown user:group file.txt",
            "echo 'data' > output.txt",  // Redirection
        ];

        for cmd in cases {
            match CommandClassifier::classify(cmd) {
                SafetyLevel::Warning => {},  // Expected
                _ => panic!("Command '{}' should be classified as Warning", cmd),
            }
        }
    }

    #[test]
    fn test_safe_commands() {
        let cases = vec![
            "ls -la",
            "pwd",
            "cat file.txt",
            "grep 'pattern' file.txt",
            "echo 'hello world'",
            "date",
            "whoami",
            "ps aux",
        ];

        for cmd in cases {
            match CommandClassifier::classify(cmd) {
                SafetyLevel::Safe => {},  // Expected
                _ => panic!("Command '{}' should be classified as Safe", cmd),
            }
        }
    }

    #[test]
    fn test_whitespace_normalization() {
        let cmd1 = "ls    -la";  // Multiple spaces
        let cmd2 = "ls -la";

        // Both should be classified the same
        assert!(matches!(CommandClassifier::classify(cmd1), SafetyLevel::Safe));
        assert!(matches!(CommandClassifier::classify(cmd2), SafetyLevel::Safe));
    }

    #[test]
    fn test_empty_command() {
        assert!(matches!(CommandClassifier::classify(""), SafetyLevel::Safe));
        assert!(matches!(CommandClassifier::classify("   "), SafetyLevel::Safe));
    }
}
