//! Advanced Bash Executor - Clawdbot-style shell execution
//!
//! Ported from: clawdbot-main/src/agents/bash-tools.exec.ts
//!
//! Features:
//! - PTY-like interactive shell support
//! - Background process tracking
//! - Approval workflow integration
//! - Process registry for long-running commands

#[path = "exec.rs"]
mod exec;
#[path = "interactive.rs"]
mod interactive;
#[path = "registry.rs"]
mod registry;

pub use self::exec::{
    exec, exec_background, exec_timeout, execute_bash, BashExecConfig, BashExecResult,
};
pub use self::interactive::InteractiveSession;
pub use self::registry::{
    get_process, kill_process, list_active_processes, register_process, update_process,
    ProcessInfo, ProcessStatus,
};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
