// Approval Gate - Ported from clawdbot-main/src/agents/bash-tools.exec.ts
// Provides command approval workflow for dangerous operations

use lazy_static::lazy_static;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// =====================================================
// Approval Types (from clawdbot)
// =====================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalLevel {
    /// Always auto-approve (safe commands)
    AutoApprove,
    /// Require user approval
    RequireApproval,
    /// Block entirely (never run)
    Blocked,
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub id: String,
    pub command: String,
    pub level: ApprovalLevel,
    pub reason: String,
    pub created_at: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

// =====================================================
// Dangerous Command Patterns (from clawdbot safeBins concept)
// =====================================================

lazy_static! {
    /// Commands that are always blocked
    static ref BLOCKED_PATTERNS: Vec<&'static str> = vec![
        "rm -rf /",
        "rm -rf /*",
        "rm -rf ~",
        "mkfs",
        ":(){:|:&};:",  // Fork bomb
        "dd if=/dev/zero",
        "chmod -R 777 /",
        "> /dev/sda",
    ];

    /// Safe binaries (conditionally auto-approved when arguments are non-risky)
    static ref SAFE_BINS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        set.insert("ls");
        set.insert("pwd");
        set.insert("echo");
        set.insert("cat");
        set.insert("head");
        set.insert("tail");
        set.insert("grep");
        set.insert("cut");
        set.insert("tr");
        set.insert("find");
        set.insert("which");
        set.insert("whoami");
        set.insert("date");
        set.insert("cal");
        set.insert("df");
        set.insert("du");
        set.insert("wc");
        set.insert("sort");
        set.insert("uniq");
        set.insert("diff");
        set.insert("env");
        set.insert("printenv");
        set
    };
    /// JSON actions that are read-only/low-risk and can auto-pass by default.
    static ref SAFE_NON_SHELL_ACTIONS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        set.insert("snapshot");
        set.insert("read");
        set.insert("read_clipboard");
        set.insert("extract");
        set.insert("wait");
        set.insert("done");
        set
    };

    /// Pending approvals registry
    static ref PENDING_APPROVALS: Mutex<Vec<ApprovalRequest>> = Mutex::new(Vec::new());
    /// User decisions keyed by plan_id + action fingerprint.
    static ref DECISION_REGISTRY: Mutex<HashMap<String, DecisionEntry>> = Mutex::new(HashMap::new());
}

#[derive(Debug, Clone)]
struct DecisionEntry {
    status: ApprovalStatus,
    expires_at: Instant,
}

// =====================================================
// Approval Gate Implementation
// =====================================================

pub struct ApprovalGate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellOperator {
    Pipe,
    And,
    Or,
    Seq,
}

#[derive(Debug, Default)]
struct ParsedShellCommand {
    segments: Vec<String>,
    operators: Vec<ShellOperator>,
    has_redirection: bool,
    has_substitution: bool,
    has_unterminated_quote: bool,
}

impl ApprovalGate {
    fn push_segment(buffer: &mut String, segments: &mut Vec<String>) {
        let trimmed = buffer.trim();
        if !trimmed.is_empty() {
            segments.push(trimmed.to_string());
        }
        buffer.clear();
    }

    fn parse_shell_command(cmd: &str) -> ParsedShellCommand {
        let mut parsed = ParsedShellCommand::default();
        let mut current = String::new();
        let mut chars = cmd.chars().peekable();
        let mut in_single = false;
        let mut in_double = false;
        let mut escaped = false;

        while let Some(ch) = chars.next() {
            if escaped {
                current.push(ch);
                escaped = false;
                continue;
            }

            if ch == '\\' && !in_single {
                escaped = true;
                current.push(ch);
                continue;
            }

            if ch == '\'' && !in_double {
                in_single = !in_single;
                current.push(ch);
                continue;
            }
            if ch == '"' && !in_single {
                in_double = !in_double;
                current.push(ch);
                continue;
            }

            if !in_single && !in_double {
                if ch == '`' {
                    parsed.has_substitution = true;
                    current.push(ch);
                    continue;
                }
                if ch == '$' && matches!(chars.peek(), Some('(')) {
                    parsed.has_substitution = true;
                    current.push(ch);
                    continue;
                }
                if ch == '>' || ch == '<' {
                    parsed.has_redirection = true;
                    current.push(ch);
                    continue;
                }
                if ch == '&' && matches!(chars.peek(), Some('&')) {
                    let _ = chars.next();
                    Self::push_segment(&mut current, &mut parsed.segments);
                    parsed.operators.push(ShellOperator::And);
                    continue;
                }
                if ch == '|' {
                    if matches!(chars.peek(), Some('|')) {
                        let _ = chars.next();
                        Self::push_segment(&mut current, &mut parsed.segments);
                        parsed.operators.push(ShellOperator::Or);
                        continue;
                    }
                    Self::push_segment(&mut current, &mut parsed.segments);
                    parsed.operators.push(ShellOperator::Pipe);
                    continue;
                }
                if ch == ';' {
                    Self::push_segment(&mut current, &mut parsed.segments);
                    parsed.operators.push(ShellOperator::Seq);
                    continue;
                }
            }

            current.push(ch);
        }

        if escaped || in_single || in_double {
            parsed.has_unterminated_quote = true;
        }
        Self::push_segment(&mut current, &mut parsed.segments);
        parsed
    }

    fn command_words(cmd: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut current = String::new();
        let chars = cmd.chars().peekable();
        let mut in_single = false;
        let mut in_double = false;
        let mut escaped = false;

        for ch in chars {
            if escaped {
                current.push(ch);
                escaped = false;
                continue;
            }

            if ch == '\\' && !in_single {
                escaped = true;
                continue;
            }

            if ch == '\'' && !in_double {
                in_single = !in_single;
                continue;
            }
            if ch == '"' && !in_single {
                in_double = !in_double;
                continue;
            }

            if ch.is_whitespace() && !in_single && !in_double {
                if !current.is_empty() {
                    words.push(current.to_lowercase());
                    current.clear();
                }
                continue;
            }
            current.push(ch);
        }

        if !current.is_empty() {
            words.push(current.to_lowercase());
        }
        words
    }

    fn command_binary(words: &[String]) -> String {
        let Some(first) = words.first() else {
            return String::new();
        };
        std::path::Path::new(first)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(first)
            .to_string()
    }

    fn requires_approval_by_tokens(binary: &str, words: &[String]) -> bool {
        if binary.is_empty() {
            return true;
        }
        if matches!(
            binary,
            "sudo"
                | "rm"
                | "chmod"
                | "chown"
                | "kill"
                | "killall"
                | "shutdown"
                | "reboot"
                | "passwd"
                | "curl"
                | "wget"
        ) {
            return true;
        }
        if matches!(
            binary,
            "apt" | "apt-get" | "brew" | "pip" | "pip3" | "npm" | "cargo"
        ) && words.iter().skip(1).any(|w| {
            matches!(
                w.as_str(),
                "install" | "i" | "upgrade" | "remove" | "uninstall" | "global" | "-g"
            )
        }) {
            return true;
        }
        false
    }

    fn is_blocked_segment(segment: &str, binary: &str, words: &[String]) -> bool {
        let seg_lc = segment.trim().to_lowercase();
        if seg_lc.is_empty() {
            return false;
        }

        for pattern in BLOCKED_PATTERNS.iter() {
            if seg_lc.starts_with(pattern) {
                return true;
            }
        }

        if binary == "mkfs" {
            return true;
        }
        if binary == "rm" {
            let has_rf = words
                .iter()
                .any(|w| w.starts_with('-') && w.contains('r') && w.contains('f'));
            let has_root_target = words
                .iter()
                .any(|w| matches!(w.as_str(), "/" | "/*" | "~" | "~/"));
            if has_rf && has_root_target {
                return true;
            }
        }
        if seg_lc.contains("/dev/sda") && seg_lc.contains('>') {
            return true;
        }

        false
    }

    fn token_looks_path_like(token: &str) -> bool {
        token.contains('/')
            || token.contains('\\')
            || token.starts_with('.')
            || token.starts_with('~')
            || token.starts_with('*')
            || token.ends_with('*')
    }

    fn safe_bin_args_ok(binary: &str, words: &[String]) -> bool {
        if words.len() <= 1 {
            return true;
        }
        for arg in words.iter().skip(1) {
            if arg.is_empty() {
                continue;
            }
            if arg.starts_with('-') {
                continue;
            }
            if arg.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            // echo is text-oriented; keep existing low-friction behavior for literals.
            if binary == "echo" {
                continue;
            }
            // Safe bins should not auto-approve positional/path-like args.
            if Self::token_looks_path_like(arg) {
                return false;
            }
            return false;
        }
        true
    }

    /// Check if a command requires approval, is blocked, or can auto-run
    pub fn check_command(cmd: &str) -> ApprovalLevel {
        let cmd_trimmed = cmd.trim();
        if cmd_trimmed.is_empty() {
            return ApprovalLevel::RequireApproval;
        }

        let parsed = Self::parse_shell_command(cmd_trimmed);
        if parsed.has_unterminated_quote {
            return ApprovalLevel::RequireApproval;
        }
        if parsed.has_substitution {
            return ApprovalLevel::RequireApproval;
        }

        if parsed.has_redirection {
            if parsed.segments.iter().any(|segment| {
                let lowered = segment.to_lowercase();
                lowered.contains("/dev/sda") && lowered.contains('>')
            }) {
                return ApprovalLevel::Blocked;
            }
            return ApprovalLevel::RequireApproval;
        }

        let mut has_required_segment = false;
        for segment in &parsed.segments {
            let words = Self::command_words(segment);
            let binary_name = Self::command_binary(&words);

            if Self::is_blocked_segment(segment, &binary_name, &words) {
                return ApprovalLevel::Blocked;
            }
            if Self::requires_approval_by_tokens(&binary_name, &words) {
                has_required_segment = true;
                continue;
            }
            if SAFE_BINS.contains(binary_name.as_str())
                && Self::safe_bin_args_ok(&binary_name, &words)
            {
                continue;
            }
            has_required_segment = true;
        }

        if has_required_segment {
            return ApprovalLevel::RequireApproval;
        }

        if parsed.operators.iter().any(|op| {
            matches!(
                op,
                ShellOperator::And | ShellOperator::Or | ShellOperator::Seq
            )
        }) {
            return ApprovalLevel::RequireApproval;
        }

        ApprovalLevel::AutoApprove
    }

    /// Create an approval request for a command
    pub fn request_approval(cmd: &str) -> ApprovalRequest {
        let level = Self::check_command(cmd);
        let reason = match level {
            ApprovalLevel::Blocked => {
                "Command contains dangerous patterns and is blocked.".to_string()
            }
            ApprovalLevel::RequireApproval => format!("Command may modify system: '{}'", cmd),
            ApprovalLevel::AutoApprove => "Safe command.".to_string(),
        };

        let request = ApprovalRequest {
            id: uuid::Uuid::new_v4().to_string(),
            command: cmd.to_string(),
            level,
            reason,
            created_at: std::time::Instant::now(),
        };

        if level == ApprovalLevel::RequireApproval {
            if let Ok(mut pending) = PENDING_APPROVALS.lock() {
                pending.push(request.clone());
            }
        }

        request
    }

    /// Approve a pending request by ID
    pub fn approve(id: &str) -> bool {
        if let Ok(mut pending) = PENDING_APPROVALS.lock() {
            if let Some(pos) = pending.iter().position(|r| r.id == id) {
                pending.remove(pos);
                return true;
            }
        }
        false
    }

    /// Deny a pending request by ID
    pub fn deny(id: &str) -> bool {
        Self::approve(id) // Same logic - just remove from pending
    }

    /// Get all pending approvals
    pub fn get_pending() -> Vec<ApprovalRequest> {
        PENDING_APPROVALS
            .lock()
            .map(|p| p.clone())
            .unwrap_or_default()
    }

    /// Clear expired approvals (older than 2 minutes)
    pub fn clear_expired() {
        if let Ok(mut pending) = PENDING_APPROVALS.lock() {
            let expiry = std::time::Duration::from_secs(120);
            pending.retain(|r| r.created_at.elapsed() < expiry);
        }
    }
}

mod support;
pub use support::{evaluate_approval, preview_approval, register_decision, ApprovalDecision};

#[cfg(test)]
mod tests;
