use std::process::{Child, Command, Stdio};
use std::io::{Write, BufReader, BufRead};
use tokio::sync::mpsc;
use tokio::task;
use crate::schema::{AgentCommand, AgentResponse};
use serde_json::json;

pub struct AdapterClient {
    tx: mpsc::Sender<AgentCommand>,
    // Rx for responses? In a real async actor system, we'd map responses to requests.
    // For MVP, simplistic Request-Response channel.
}

impl AdapterClient {
    pub fn spawn() -> anyhow::Result<(mpsc::Sender<AgentCommand>, mpsc::Receiver<AgentResponse>)> {
        // Path to the adapter executable
        // NOTE: In dev, assume it's in a known relative path or built target
        let adapter_path = "../adapter/.build/debug/adapter";

        let mut child = Command::new(adapter_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Log adapter stderr to parent stderr
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn adapter at {}: {}", adapter_path, e))?;

        println!("Adapter process spawned with PID: {}", child.id());

        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        let stdout = child.stdout.take().expect("Failed to open stdout");

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<AgentCommand>(32);
        let (resp_tx, resp_rx) = mpsc::channel::<AgentResponse>(32);

        // Writer Task
        task::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                if let Ok(json_str) = serde_json::to_string(&cmd) {
                    let _ = stdin.write_all(json_str.as_bytes());
                    let _ = stdin.write_all(b"\n");
                    let _ = stdin.flush();
                }
            }
        });

        // Reader Task
        task::spawn(async move {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(l) = line {
                    if let Ok(resp) = serde_json::from_str::<AgentResponse>(&l) {
                        let _ = resp_tx.send(resp).await;
                    } else {
                        eprintln!("[IPC Error] Failed to parse swift output: {}", l);
                    }
                }
            }
        });

        Ok((cmd_tx, resp_rx))
    }
}
