use super::super::*;

impl N8nApi {
    pub async fn create_workflow(
        &self,
        name: &str,
        workflow_json: &Value,
        active: bool,
    ) -> Result<String> {
        if crate::env_flag("STEER_N8N_MOCK") {
            let mock_id = format!("mock-wf-{}", chrono::Utc::now().timestamp_millis());
            println!(
                "🧪 STEER_N8N_MOCK=1: skipping n8n network/CLI calls and returning {}",
                mock_id
            );
            return Ok(mock_id);
        }

        // 1. Validate JSON and normalize once for consistent downstream behavior.
        let normalized = normalize_workflow_for_create(name, workflow_json)?;

        // 2. Validate Credentials (Prevent broken workflows)
        // Only if API key is present (we need API to list creds)
        if !self.api_key.is_empty() && self.api_key != "placeholder" {
            if let Ok(creds) = self.list_credentials().await {
                let valid_ids: Vec<String> = creds.iter().map(|c| c.id.clone()).collect();

                if let Some(nodes) = normalized.get("nodes").and_then(|n| n.as_array()) {
                    for node in nodes {
                        if let Some(cred_map) = node.get("credentials") {
                            if let Some(obj) = cred_map.as_object() {
                                for (_, v) in obj {
                                    if let Some(id) = v.get("id").and_then(|i: &Value| i.as_str()) {
                                        if !valid_ids.contains(&id.to_string()) {
                                            return Err(anyhow::anyhow!("❌ Validation Failed: Credential ID '{}' does not exist in n8n.", id));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let runtime = self.runtime_mode();
        let allow_cli_fallback = self.cli_fallback_enabled(runtime);

        // 3. Try API
        if !self.api_key.is_empty() && self.api_key != "placeholder" {
            println!("🌐 Attempting to create workflow via API...");
            match self.create_workflow_api(name, &normalized, active).await {
                Ok(id) => return Ok(id),
                Err(e) => {
                    if !allow_cli_fallback {
                        return Err(anyhow::anyhow!(
                            "n8n API creation failed and CLI fallback is disabled (runtime={}): {}",
                            runtime.as_str(),
                            e
                        ));
                    }
                    println!("⚠️ API creation failed ({}). Falling back to CLI...", e);
                }
            }
        } else {
            if !allow_cli_fallback {
                return Err(anyhow::anyhow!(
                    "N8N_API_KEY is not set and CLI fallback is disabled (runtime={}). Set N8N_API_KEY or enable STEER_N8N_ALLOW_CLI_FALLBACK=1",
                    runtime.as_str()
                ));
            }
            println!("ℹ️ No API Key configured. Using CLI fallback mode.");
        }

        // 4. Fallback to CLI (Strict Local Check)
        if !self.local_target() {
            return Err(anyhow::anyhow!(
                "❌ CLI Fallback aborted: n8n is remote ({}). CLI only works for local instances.",
                self.base_url
            ));
        }

        let import_marker = format!("steer-import-{}", uuid::Uuid::new_v4());

        // 5. Run CLI Import
        if let Err(e) = self
            .create_workflow_cli(name, &normalized, active, &import_marker)
            .await
        {
            return Err(anyhow::anyhow!("❌ CLI Fallback Failed: {}", e));
        }

        // 6. Retrieve ID via CLI export (no direct SQLite coupling).
        self.retrieve_workflow_id_via_cli_export(name, &import_marker)
            .await
    }

    async fn create_workflow_api(
        &self,
        name: &str,
        workflow_json: &Value,
        active: bool,
    ) -> Result<String> {
        let url = format!("{}/workflows", self.base_url);

        let body = json!({
            "name": name,
            "nodes": workflow_json.get("nodes").cloned().unwrap_or(json!([])),
            "connections": workflow_json.get("connections").cloned().unwrap_or(json!({})),
            "settings": workflow_json.get("settings").cloned().unwrap_or(json!({"saveManualExecutions": true}))
        });
        // NOTE: Some n8n versions reject `active` as read-only on create.
        // We always create inactive here; activation can be done via a separate endpoint if needed.

        let body_for_req = body.clone();
        let resp = self
            .send_with_retry("n8n create_workflow", || {
                self.client
                    .post(&url)
                    .header("X-N8N-API-KEY", &self.api_key)
                    .json(&body_for_req)
            })
            .await?;

        if !resp.status().is_success() {
            let error_text = resp.text().await?;
            return Err(anyhow::anyhow!("n8n API Error: {}", error_text));
        }

        let resp_json: Value = resp.json().await?;
        let id = resp_json["id"].as_str().unwrap_or("unknown").to_string();
        if let Err(webhook_fix_err) = self.enable_disabled_webhook_nodes(&id).await {
            eprintln!(
                "⚠️ failed to normalize webhook trigger state for workflow {}: {}",
                id, webhook_fix_err
            );
        }
        if active {
            if let Err(activate_err) = self.activate_workflow(&id).await {
                // Some n8n versions expose activation differently; attempt generic update fallback.
                self.update_workflow_active(&id, true).await.map_err(|patch_err| {
                    anyhow::anyhow!(
                        "workflow created (id={}) but activation failed: {} | fallback update failed: {}",
                        id,
                        activate_err,
                        patch_err
                    )
                })?;
            }
        }
        Ok(id)
    }

    pub(super) async fn enable_disabled_webhook_nodes(&self, id: &str) -> Result<bool> {
        let url = format!("{}/workflows/{}", self.base_url, id);
        let workflow_resp = self
            .send_with_retry("n8n get_workflow_for_webhook_fix", || {
                self.client.get(&url).header("X-N8N-API-KEY", &self.api_key)
            })
            .await?;
        let workflow: Value = workflow_resp.json().await?;

        let nodes = workflow
            .get("nodes")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("workflow {} has invalid nodes payload", id))?;

        let mut has_webhook = false;
        let mut changed = false;
        let mut normalized_nodes: Vec<Value> = Vec::with_capacity(nodes.len());
        for node in nodes {
            let mut node_value = node.clone();
            if node
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                == "n8n-nodes-base.webhook"
            {
                has_webhook = true;
                if let Some(obj) = node_value.as_object_mut() {
                    let is_disabled = obj
                        .get("disabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if is_disabled {
                        obj.insert("disabled".to_string(), Value::Bool(false));
                        changed = true;
                    }
                    let missing_webhook_id = obj
                        .get("webhookId")
                        .and_then(|v| v.as_str())
                        .map(|v| v.trim().is_empty())
                        .unwrap_or(true);
                    if missing_webhook_id {
                        obj.insert(
                            "webhookId".to_string(),
                            Value::String(uuid::Uuid::new_v4().to_string()),
                        );
                        changed = true;
                    }
                }
            }
            normalized_nodes.push(node_value);
        }

        if !has_webhook || !changed {
            return Ok(false);
        }

        let update_body = json!({
            "name": workflow.get("name").cloned().unwrap_or_else(|| json!("workflow")),
            "nodes": normalized_nodes,
            "connections": workflow.get("connections").cloned().unwrap_or_else(|| json!({})),
            "settings": workflow.get("settings").cloned().unwrap_or_else(|| json!({}))
        });
        let _ = self
            .send_with_retry("n8n fix_disabled_webhook_nodes", || {
                self.client
                    .put(&url)
                    .header("X-N8N-API-KEY", &self.api_key)
                    .json(&update_body)
            })
            .await?;
        Ok(true)
    }

    async fn update_workflow_active(&self, id: &str, active: bool) -> Result<()> {
        let url = format!("{}/workflows/{}", self.base_url, id);
        let workflow_resp = self
            .send_with_retry("n8n get_workflow_for_active_update", || {
                self.client.get(&url).header("X-N8N-API-KEY", &self.api_key)
            })
            .await?;
        let workflow: Value = workflow_resp.json().await?;
        let update_body = json!({
            "name": workflow.get("name").cloned().unwrap_or_else(|| json!("workflow")),
            "nodes": workflow.get("nodes").cloned().unwrap_or_else(|| json!([])),
            "connections": workflow.get("connections").cloned().unwrap_or_else(|| json!({})),
            "settings": workflow.get("settings").cloned().unwrap_or_else(|| json!({})),
            "active": active
        });
        let _ = self
            .send_with_retry("n8n update_workflow_active", || {
                self.client
                    .put(&url)
                    .header("X-N8N-API-KEY", &self.api_key)
                    .json(&update_body)
            })
            .await?;
        if active {
            let _ = self.enable_disabled_webhook_nodes(id).await;
        }
        Ok(())
    }

    async fn create_workflow_cli(
        &self,
        name: &str,
        workflow_json: &Value,
        active: bool,
        import_marker: &str,
    ) -> Result<String> {
        // Prepare JSON file
        let mut final_json = workflow_json.clone();
        final_json["name"] = json!(name);
        final_json["active"] = json!(active);
        let mut settings = final_json
            .get("settings")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !settings.is_object() {
            settings = json!({});
        }
        settings["steerImportId"] = json!(import_marker);
        final_json["settings"] = settings;

        // Ensure nodes exist
        if final_json["nodes"].as_array().is_none_or(|n| n.is_empty()) {
            return Err(anyhow::anyhow!("Refusing to import empty workflow via CLI"));
        }

        let path = format!("/tmp/n8n_import_{}.json", uuid::Uuid::new_v4());
        tokio::fs::write(&path, serde_json::to_string(&final_json)?).await?;

        println!("📥 Importing workflow via CLI from {}...", path);

        let (cli_bin, cli_prefix) = self.resolve_cli_invocation()?;
        let mut cmd = tokio::process::Command::new(&cli_bin);
        cmd.args(&cli_prefix);
        cmd.args(["import:workflow", "--input", &path]);
        let output = cmd.output().await?;

        // Cleanup
        if let Err(e) = tokio::fs::remove_file(&path).await {
            eprintln!("⚠️ Failed to clean up temp file: {}", e);
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let code = output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string());
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            return Err(anyhow::anyhow!(
                "CLI Import failed (bin={}, exit {}): {}",
                cli_bin,
                code,
                detail
            ));
        }

        println!("✅ CLI Import successful!");

        Ok("cli-imported".to_string())
    }

    fn workflow_id_from_value(value: Option<&Value>) -> Option<String> {
        match value {
            Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        }
    }

    fn export_items_from_value(value: &Value) -> Vec<Value> {
        match value {
            Value::Array(items) => items.clone(),
            Value::Object(map) => {
                if let Some(Value::Array(items)) = map.get("data") {
                    items.clone()
                } else if map.contains_key("nodes") || map.contains_key("connections") {
                    vec![value.clone()]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    async fn retrieve_workflow_id_via_cli_export(
        &self,
        name: &str,
        import_marker: &str,
    ) -> Result<String> {
        let path = format!("/tmp/n8n_export_{}.json", uuid::Uuid::new_v4());
        let (cli_bin, cli_prefix) = self.resolve_cli_invocation()?;
        let mut cmd = tokio::process::Command::new(&cli_bin);
        cmd.args(&cli_prefix);
        cmd.args(["export:workflow", "--all", "--output", &path]);
        let output = cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            return Err(anyhow::anyhow!(
                "CLI workflow id lookup failed after import (bin={}): {}",
                cli_bin,
                detail
            ));
        }

        let raw = tokio::fs::read_to_string(&path).await?;
        if let Err(e) = tokio::fs::remove_file(&path).await {
            eprintln!("⚠️ Failed to clean up export file {}: {}", path, e);
        }

        let parsed: Value = serde_json::from_str(&raw).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse exported workflows while resolving imported workflow id: {}",
                e
            )
        })?;
        let items = Self::export_items_from_value(&parsed);
        if items.is_empty() {
            return Err(anyhow::anyhow!(
                "No workflows found in n8n CLI export while resolving imported workflow id"
            ));
        }

        let mut marker_match: Option<String> = None;
        let mut name_matches: Vec<String> = Vec::new();
        for item in items {
            let Some(id) = Self::workflow_id_from_value(item.get("id")) else {
                continue;
            };
            let settings_text = item
                .get("settings")
                .map(|v| v.to_string())
                .unwrap_or_default();
            if !import_marker.trim().is_empty() && settings_text.contains(import_marker) {
                marker_match = Some(id);
                break;
            }

            if item
                .get("name")
                .and_then(|v| v.as_str())
                .map(|wf_name| wf_name == name)
                .unwrap_or(false)
            {
                name_matches.push(id);
            }
        }

        if let Some(id) = marker_match {
            return Ok(id);
        }
        let allow_name_fallback =
            parse_bool_env_with_default("STEER_N8N_ALLOW_NAME_ID_FALLBACK", false);
        if allow_name_fallback {
            if let Some(id) = name_matches.first() {
                if name_matches.len() > 1
                    && !parse_bool_env_with_default(
                        "STEER_N8N_ALLOW_AMBIGUOUS_NAME_ID_FALLBACK",
                        false,
                    )
                {
                    return Err(anyhow::anyhow!(
                        "Ambiguous name-based fallback for '{}': {} matches. \
Set STEER_N8N_ALLOW_AMBIGUOUS_NAME_ID_FALLBACK=1 only for controlled test environments.",
                        name,
                        name_matches.len()
                    ));
                }
                if name_matches.len() > 1 {
                    eprintln!(
                        "⚠️ Ambiguous name fallback explicitly allowed for '{}'; using first exported id={}",
                        name, id
                    );
                }
                return Ok(id.clone());
            }
        }

        if !name_matches.is_empty() {
            return Err(anyhow::anyhow!(
                "Import marker not found in exported workflows; {} name match(es) exist for '{}'. \
Set STEER_N8N_ALLOW_NAME_ID_FALLBACK=1 to allow name-based fallback.",
                name_matches.len(),
                name
            ));
        }
        Err(anyhow::anyhow!(
            "Could not resolve imported workflow id via n8n CLI export"
        ))
    }

    fn build_minimal_workflow(name: &str) -> Value {
        build_orchestrator_fallback_workflow(name, None, "legacy_minimal_template")
    }
}
