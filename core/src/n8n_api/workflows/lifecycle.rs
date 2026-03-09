use super::super::*;

impl N8nApi {
    async fn activate_workflow_cli(&self, id: &str, active: bool) -> Result<()> {
        let (cli_bin, cli_prefix) = self.resolve_cli_invocation()?;
        let mut cmd = tokio::process::Command::new(&cli_bin);
        cmd.args(&cli_prefix);
        cmd.args([
            "update:workflow",
            "--id",
            id,
            "--active",
            if active { "true" } else { "false" },
        ]);
        let output = cmd.output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            return Err(anyhow::anyhow!(
                "n8n CLI update:workflow failed for id={} active={}: {}",
                id,
                active,
                detail
            ));
        }
        Ok(())
    }

    pub async fn activate_workflow(&self, id: &str) -> Result<()> {
        let url = format!("{}/workflows/{}/activate", self.base_url, id);
        match self
            .send_with_retry("n8n activate_workflow", || {
                self.client
                    .post(&url)
                    .header("X-N8N-API-KEY", &self.api_key)
            })
            .await
        {
            Ok(_) => Ok(()),
            Err(api_err) => {
                if self.local_target() {
                    if let Ok(true) = self.enable_disabled_webhook_nodes(id).await {
                        match self
                            .send_with_retry(
                                "n8n activate_workflow_retry_after_webhook_fix",
                                || {
                                    self.client
                                        .post(&url)
                                        .header("X-N8N-API-KEY", &self.api_key)
                                },
                            )
                            .await
                        {
                            Ok(_) => return Ok(()),
                            Err(retry_err) => {
                                eprintln!(
                                    "⚠️ activate retry after webhook-fix failed: {}",
                                    retry_err
                                );
                            }
                        }
                    }
                    match self.activate_workflow_cli(id, true).await {
                        Ok(_) => {
                            println!(
                                "⚠️ n8n API activate failed ({}), recovered with CLI update:workflow",
                                api_err
                            );
                            return Ok(());
                        }
                        Err(cli_err) => {
                            return Err(anyhow::anyhow!(
                                "n8n activate error: {} | CLI fallback failed: {}",
                                api_err,
                                cli_err
                            ));
                        }
                    }
                }
                Err(anyhow::anyhow!("n8n activate error: {}", api_err))
            }
        }
    }

    /// Deactivate a workflow
    pub async fn deactivate_workflow(&self, id: &str) -> Result<()> {
        let url = format!("{}/workflows/{}/deactivate", self.base_url, id);
        match self
            .send_with_retry("n8n deactivate_workflow", || {
                self.client
                    .post(&url)
                    .header("X-N8N-API-KEY", &self.api_key)
            })
            .await
        {
            Ok(_) => Ok(()),
            Err(api_err) => {
                if self.local_target() {
                    match self.activate_workflow_cli(id, false).await {
                        Ok(_) => {
                            println!(
                                "⚠️ n8n API deactivate failed ({}), recovered with CLI update:workflow",
                                api_err
                            );
                            return Ok(());
                        }
                        Err(cli_err) => {
                            return Err(anyhow::anyhow!(
                                "n8n deactivate error: {} | CLI fallback failed: {}",
                                api_err,
                                cli_err
                            ));
                        }
                    }
                }
                Err(anyhow::anyhow!("n8n deactivate error: {}", api_err))
            }
        }
    }
}
