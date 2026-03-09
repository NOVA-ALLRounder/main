use super::registry::{register_process, update_process, ProcessStatus};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct BashExecConfig {
    pub timeout_ms: u64,
    pub working_dir: Option<String>,
    pub env_vars: HashMap<String, String>,
    pub background: bool,
    pub approval_required: bool,
}

impl Default for BashExecConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 30000,
            working_dir: None,
            env_vars: HashMap::new(),
            background: false,
            approval_required: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BashExecResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub process_id: Option<String>,
}

fn should_allow_test_auto_approval(
    explicit_auto_approval: bool,
    test_assume_approved: bool,
) -> bool {
    explicit_auto_approval && test_assume_approved
}

pub fn execute_bash(cmd: &str, config: &BashExecConfig) -> Result<BashExecResult> {
    use crate::approval_gate::{ApprovalGate, ApprovalLevel};

    let start = Instant::now();

    if config.approval_required {
        match ApprovalGate::check_command(cmd) {
            ApprovalLevel::Blocked => {
                return Ok(BashExecResult {
                    success: false,
                    stdout: String::new(),
                    stderr: "Command blocked by security policy".to_string(),
                    exit_code: -1,
                    duration_ms: 0,
                    process_id: None,
                });
            }
            ApprovalLevel::RequireApproval => {
                let explicit_auto_approval = crate::env_flag("STEER_BASH_ALLOW_AUTO_APPROVAL");
                let test_assume_approved = crate::env_flag("STEER_TEST_ASSUME_APPROVED");
                if should_allow_test_auto_approval(explicit_auto_approval, test_assume_approved) {
                    println!(
                        "🧪 [Bash] Auto-approved in test mode (STEER_BASH_ALLOW_AUTO_APPROVAL=1, STEER_TEST_ASSUME_APPROVED=1): {}",
                        cmd
                    );
                } else if explicit_auto_approval {
                    return Ok(BashExecResult {
                        success: false,
                        stdout: String::new(),
                        stderr: "Auto-approval disabled outside test mode (set STEER_TEST_ASSUME_APPROVED=1 for test-only bypass)".to_string(),
                        exit_code: -2,
                        duration_ms: 0,
                        process_id: None,
                    });
                } else {
                    return Ok(BashExecResult {
                        success: false,
                        stdout: String::new(),
                        stderr: "Command requires explicit approval".to_string(),
                        exit_code: -2,
                        duration_ms: 0,
                        process_id: None,
                    });
                }
            }
            ApprovalLevel::AutoApprove => {}
        }
    }

    if config.background {
        return execute_background(cmd, config);
    }

    let mut command = Command::new("/bin/bash");
    command.arg("-c").arg(cmd);

    if let Some(dir) = &config.working_dir {
        command.current_dir(dir);
    }

    for (key, value) in &config.env_vars {
        command.env(key, value);
    }

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let child = command
        .spawn()
        .context(format!("Failed to spawn command: {}", cmd))?;

    let output = wait_with_timeout(child, Duration::from_millis(config.timeout_ms))
        .context("Command execution failed")?;

    let duration = start.elapsed().as_millis() as u64;

    Ok(BashExecResult {
        success: output.0,
        stdout: output.1,
        stderr: output.2,
        exit_code: output.3,
        duration_ms: duration,
        process_id: None,
    })
}

fn execute_background(cmd: &str, config: &BashExecConfig) -> Result<BashExecResult> {
    let mut command = Command::new("/bin/bash");
    command.arg("-c").arg(cmd);

    if let Some(dir) = &config.working_dir {
        command.current_dir(dir);
    }

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let child = command
        .spawn()
        .context(format!("Failed to spawn background command: {}", cmd))?;

    let pid = child.id();
    let process_id = format!("bg_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    register_process(&process_id, pid, cmd);

    let proc_id = process_id.clone();
    std::thread::spawn(move || {
        let output = child.wait_with_output();
        match output {
            Ok(out) => {
                let status = if out.status.success() {
                    ProcessStatus::Completed
                } else {
                    ProcessStatus::Failed
                };
                let stdout_lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(|s| s.to_string())
                    .collect();
                update_process(&proc_id, status, out.status.code(), Some(stdout_lines));
            }
            Err(_) => {
                update_process(&proc_id, ProcessStatus::Failed, Some(-1), None);
            }
        }
    });

    Ok(BashExecResult {
        success: true,
        stdout: format!("Background process started: {}", process_id),
        stderr: String::new(),
        exit_code: 0,
        duration_ms: 0,
        process_id: Some(process_id),
    })
}

fn wait_with_timeout(child: Child, timeout: Duration) -> Result<(bool, String, String, i32)> {
    use std::sync::mpsc;
    use std::thread;

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => {
            let output = result?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);
            Ok((output.status.success(), stdout, stderr, exit_code))
        }
        Err(_) => Err(anyhow::anyhow!(
            "Command timed out after {}ms",
            timeout.as_millis()
        )),
    }
}

pub fn exec(cmd: &str) -> Result<String> {
    let config = BashExecConfig {
        approval_required: true,
        ..Default::default()
    };

    let result = execute_bash(cmd, &config)?;

    if result.success {
        Ok(result.stdout)
    } else {
        Err(anyhow::anyhow!("Command failed: {}", result.stderr))
    }
}

pub fn exec_timeout(cmd: &str, timeout_ms: u64) -> Result<String> {
    let config = BashExecConfig {
        timeout_ms,
        approval_required: true,
        ..Default::default()
    };

    let result = execute_bash(cmd, &config)?;

    if result.success {
        Ok(result.stdout)
    } else {
        Err(anyhow::anyhow!("Command failed: {}", result.stderr))
    }
}

pub fn exec_background(cmd: &str) -> Result<String> {
    let config = BashExecConfig {
        background: true,
        approval_required: true,
        ..Default::default()
    };

    let result = execute_bash(cmd, &config)?;

    if let Some(id) = result.process_id {
        Ok(format!("Background process started: {}", id))
    } else {
        Ok(result.stdout)
    }
}

#[cfg(test)]
pub(super) fn should_allow_test_auto_approval_for_tests(
    explicit_auto_approval: bool,
    test_assume_approved: bool,
) -> bool {
    should_allow_test_auto_approval(explicit_auto_approval, test_assume_approved)
}
