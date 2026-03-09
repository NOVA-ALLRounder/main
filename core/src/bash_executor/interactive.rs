use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct InteractiveSession {
    child: Option<Child>,
    pub session_id: String,
}

impl InteractiveSession {
    pub fn new() -> Result<Self> {
        let child = Command::new("/bin/bash")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to start interactive bash session")?;

        let session_id = format!("pty_{}", &uuid::Uuid::new_v4().to_string()[..8]);

        println!("🖥️ [PTY] Started interactive session: {}", session_id);

        Ok(Self {
            child: Some(child),
            session_id,
        })
    }

    pub fn send(&mut self, input: &str) -> Result<String> {
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Session not running"))?;

        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No stdin available"))?;

        writeln!(stdin, "{}", input)?;
        stdin.flush()?;

        std::thread::sleep(Duration::from_millis(100));

        Ok(format!("Sent: {}", input))
    }

    pub fn close(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            println!("🖥️ [PTY] Closed session: {}", self.session_id);
        }
        Ok(())
    }
}

impl Drop for InteractiveSession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
