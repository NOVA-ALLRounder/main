use super::super::*;

impl N8nApi {
    async fn execute_workflow_cli(&self, id: &str) -> Result<ExecutionResult> {
        let (cli_bin, cli_prefix) = self.resolve_cli_invocation()?;
        let mut cmd = tokio::process::Command::new(&cli_bin);
        cmd.args(&cli_prefix);
        let broker_port = std::env::var("N8N_RUNNERS_BROKER_PORT")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .or_else(|| Self::reserve_ephemeral_local_port().map(|port| port.to_string()));
        if let Some(port) = broker_port {
            cmd.env("N8N_RUNNERS_BROKER_PORT", port);
        }
        cmd.args(["execute", "--id", id, "--rawOutput"]);
        let output = cmd.output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            return Err(anyhow::anyhow!(
                "n8n CLI execute failed for id={}: {}",
                id,
                detail
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let parsed = serde_json::from_str::<Value>(&stdout).ok();
        let now = chrono::Utc::now().to_rfc3339();
        Ok(ExecutionResult {
            id: parsed
                .as_ref()
                .and_then(|v| v.get("id").and_then(|x| x.as_str()))
                .unwrap_or("")
                .to_string(),
            finished: parsed
                .as_ref()
                .and_then(|v| v.get("finished").and_then(|x| x.as_bool()))
                .unwrap_or(true),
            status: parsed
                .as_ref()
                .and_then(|v| v.get("status").and_then(|x| x.as_str()))
                .unwrap_or("success")
                .to_string(),
            started_at: parsed
                .as_ref()
                .and_then(|v| v.get("startedAt").and_then(|x| x.as_str()))
                .unwrap_or(&now)
                .to_string(),
            stopped_at: parsed
                .as_ref()
                .and_then(|v| v.get("stoppedAt").and_then(|x| x.as_str()))
                .map(|s| s.to_string()),
        })
    }

    /// Activate a workflow
    pub async fn get_workflow(&self, id: &str) -> Result<WorkflowStatus> {
        let url = format!("{}/workflows/{}", self.base_url, id);
        let resp = self
            .send_with_retry("n8n get_workflow", || {
                self.client.get(&url).header("X-N8N-API-KEY", &self.api_key)
            })
            .await?;

        if !resp.status().is_success() {
            let error_text = resp.text().await?;
            return Err(anyhow::anyhow!("n8n get workflow error: {}", error_text));
        }

        let data: Value = resp.json().await?;
        Ok(WorkflowStatus {
            id: data["id"].as_str().unwrap_or("").to_string(),
            name: data["name"].as_str().unwrap_or("").to_string(),
            active: data["active"].as_bool().unwrap_or(false),
            created_at: data["createdAt"].as_str().unwrap_or("").to_string(),
            updated_at: data["updatedAt"].as_str().unwrap_or("").to_string(),
        })
    }

    /// List all workflows
    pub async fn list_workflows(&self) -> Result<Vec<WorkflowStatus>> {
        let url = format!("{}/workflows", self.base_url);
        let resp = self
            .send_with_retry("n8n list_workflows", || {
                self.client.get(&url).header("X-N8N-API-KEY", &self.api_key)
            })
            .await?;

        if !resp.status().is_success() {
            let error_text = resp.text().await?;
            return Err(anyhow::anyhow!("n8n list workflows error: {}", error_text));
        }

        let data: Value = resp.json().await?;
        let workflows = data["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|w| WorkflowStatus {
                        id: w["id"].as_str().unwrap_or("").to_string(),
                        name: w["name"].as_str().unwrap_or("").to_string(),
                        active: w["active"].as_bool().unwrap_or(false),
                        created_at: w["createdAt"].as_str().unwrap_or("").to_string(),
                        updated_at: w["updatedAt"].as_str().unwrap_or("").to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(workflows)
    }

    /// Execute a workflow manually
    pub async fn execute_workflow(&self, id: &str) -> Result<ExecutionResult> {
        let url = format!("{}/workflows/{}/run", self.base_url, id);
        let run_body = json!({});
        let resp = match self
            .send_with_retry("n8n execute_workflow", || {
                self.client
                    .post(&url)
                    .header("X-N8N-API-KEY", &self.api_key)
                    .json(&run_body)
            })
            .await
        {
            Ok(resp) => resp,
            Err(api_err) => {
                if self.local_target() {
                    match self.execute_workflow_cli(id).await {
                        Ok(execution) => {
                            println!(
                                "⚠️ n8n API execute failed ({}), recovered with CLI execute",
                                api_err
                            );
                            let _ = self.enable_disabled_webhook_nodes(id).await;
                            return Ok(execution);
                        }
                        Err(cli_err) => {
                            return Err(anyhow::anyhow!(
                                "n8n execute error: {} | CLI fallback failed: {}",
                                api_err,
                                cli_err
                            ));
                        }
                    }
                }
                return Err(anyhow::anyhow!("n8n execute error: {}", api_err));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await?;
            if self.local_target() {
                match self.execute_workflow_cli(id).await {
                    Ok(execution) => {
                        println!(
                            "⚠️ n8n API execute failed ({}), recovered with CLI execute",
                            error_text
                        );
                        let _ = self.enable_disabled_webhook_nodes(id).await;
                        return Ok(execution);
                    }
                    Err(cli_err) => {
                        return Err(anyhow::anyhow!(
                            "n8n execute error: {} | CLI fallback failed: {}",
                            error_text,
                            cli_err
                        ));
                    }
                }
            }
            return Err(anyhow::anyhow!("n8n execute error: {}", error_text));
        }

        let data: Value = resp.json().await?;
        let result = ExecutionResult {
            id: data["id"].as_str().unwrap_or("").to_string(),
            finished: data["finished"].as_bool().unwrap_or(false),
            status: data["status"].as_str().unwrap_or("unknown").to_string(),
            started_at: data["startedAt"].as_str().unwrap_or("").to_string(),
            stopped_at: data["stoppedAt"].as_str().map(|s| s.to_string()),
        };
        let _ = self.enable_disabled_webhook_nodes(id).await;
        Ok(result)
    }

    /// List executions for a workflow
    pub async fn list_executions(
        &self,
        workflow_id: &str,
        limit: u32,
    ) -> Result<Vec<ExecutionResult>> {
        let url = format!(
            "{}/executions?workflowId={}&limit={}",
            self.base_url, workflow_id, limit
        );

        let resp = self
            .send_with_retry("n8n list_executions", || {
                self.client.get(&url).header("X-N8N-API-KEY", &self.api_key)
            })
            .await?;

        if !resp.status().is_success() {
            let error_text = resp.text().await?;
            return Err(anyhow::anyhow!("n8n list executions error: {}", error_text));
        }

        let data: Value = resp.json().await?;
        let executions = data["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|e| ExecutionResult {
                        id: e["id"].as_str().unwrap_or("").to_string(),
                        finished: e["finished"].as_bool().unwrap_or(false),
                        status: e["status"].as_str().unwrap_or("unknown").to_string(),
                        started_at: e["startedAt"].as_str().unwrap_or("").to_string(),
                        stopped_at: e["stoppedAt"].as_str().map(|s| s.to_string()),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(executions)
    }

    /// Delete a workflow
    pub async fn delete_workflow(&self, id: &str) -> Result<()> {
        let url = format!("{}/workflows/{}", self.base_url, id);
        let resp = self
            .send_with_retry("n8n delete_workflow", || {
                self.client
                    .delete(&url)
                    .header("X-N8N-API-KEY", &self.api_key)
            })
            .await?;

        if !resp.status().is_success() {
            let error_text = resp.text().await?;
            return Err(anyhow::anyhow!("n8n delete error: {}", error_text));
        }
        Ok(())
    }
}
