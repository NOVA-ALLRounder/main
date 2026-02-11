// Workflow Builder - n8n integration for workflow automation

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Workflow Builder - Creates and manages n8n workflows
pub struct WorkflowBuilder {
    n8n_url: Option<String>,
    api_key: Option<String>,
    workflows: Arc<RwLock<Vec<Workflow>>>,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: Option<String>,
    pub name: String,
    pub nodes: Vec<WorkflowNode>,
    pub connections: HashMap<String, Vec<Connection>>,
    pub active: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub name: String,
    pub node_type: String,
    pub type_version: i32,
    pub position: (i32, i32),
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub node: String,
    pub node_type: String,
    pub index: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub template_type: TemplateType,
    pub trigger: TriggerConfig,
    pub actions: Vec<ActionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TemplateType {
    Sequential,    // Actions run in sequence
    Conditional,   // If-then logic
    Scheduled,     // Cron-based trigger
    WebhookBased,  // HTTP trigger
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerConfig {
    pub trigger_type: String,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionConfig {
    pub action_type: String,
    pub target: String,
    pub parameters: HashMap<String, String>,
}

impl WorkflowBuilder {
    pub async fn new() -> Result<Self> {
        let n8n_url = std::env::var("N8N_URL").ok();
        let api_key = std::env::var("N8N_API_KEY").ok();

        if n8n_url.is_none() {
            log::warn!("N8N_URL not set - workflow features will be limited");
        }

        Ok(Self {
            n8n_url,
            api_key,
            workflows: Arc::new(RwLock::new(Vec::new())),
            client: reqwest::Client::new(),
        })
    }

    /// Create workflow from detected pattern
    pub async fn create_from_pattern(
        &self,
        pattern_id: &str,
        actions: &[crate::jarvis::engines::pattern_detector::UserAction],
    ) -> Result<Workflow> {
        log::info!("Creating workflow from pattern: {}", pattern_id);

        // Convert actions to workflow nodes
        let mut nodes = Vec::new();
        let mut connections = HashMap::new();

        // Add start trigger node
        nodes.push(WorkflowNode {
            id: "trigger".to_string(),
            name: "Manual Trigger".to_string(),
            node_type: "n8n-nodes-base.manualTrigger".to_string(),
            type_version: 1,
            position: (100, 100),
            parameters: serde_json::json!({}),
        });

        // Convert each action to a node
        for (i, action) in actions.iter().enumerate() {
            let node = self.action_to_node(action, i)?;
            nodes.push(node);

            // Create connection from previous node
            let prev_node = if i == 0 {
                "trigger".to_string()
            } else {
                format!("action_{}", i - 1)
            };

            connections.insert(
                prev_node,
                vec![Connection {
                    node: format!("action_{}", i),
                    node_type: "main".to_string(),
                    index: 0,
                }],
            );
        }

        let workflow = Workflow {
            id: None,
            name: format!("Auto Workflow - {}", pattern_id),
            nodes,
            connections,
            active: false,
            tags: vec!["auto-generated".to_string(), pattern_id.to_string()],
        };

        // Store workflow
        self.workflows.write().await.push(workflow.clone());

        Ok(workflow)
    }

    /// Create workflow from template
    pub async fn create_from_template(&self, template: WorkflowTemplate) -> Result<Workflow> {
        log::info!("Creating workflow from template: {:?}", template.template_type);

        let mut nodes = Vec::new();
        let mut connections = HashMap::new();

        // Add trigger node
        let trigger_node = self.create_trigger_node(&template.trigger)?;
        nodes.push(trigger_node);

        // Add action nodes
        for (i, action) in template.actions.iter().enumerate() {
            let node = self.create_action_node(action, i)?;
            nodes.push(node);

            let prev_node = if i == 0 {
                "trigger".to_string()
            } else {
                format!("action_{}", i - 1)
            };

            connections.insert(
                prev_node,
                vec![Connection {
                    node: format!("action_{}", i),
                    node_type: "main".to_string(),
                    index: 0,
                }],
            );
        }

        let workflow = Workflow {
            id: None,
            name: format!("Workflow - {:?}", template.template_type),
            nodes,
            connections,
            active: false,
            tags: vec![format!("{:?}", template.template_type).to_lowercase()],
        };

        self.workflows.write().await.push(workflow.clone());

        Ok(workflow)
    }

    /// Deploy workflow to n8n
    pub async fn deploy(&self, workflow: &Workflow) -> Result<String> {
        let n8n_url = self
            .n8n_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("N8N_URL not configured"))?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("N8N_API_KEY not configured"))?;

        let url = format!("{}/api/v1/workflows", n8n_url);

        let response = self
            .client
            .post(&url)
            .header("X-N8N-API-KEY", api_key)
            .json(&workflow)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("n8n API error: {}", error_text));
        }

        let result: serde_json::Value = response.json().await?;
        let workflow_id = result["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No workflow ID in response"))?
            .to_string();

        log::info!("Workflow deployed with ID: {}", workflow_id);

        Ok(workflow_id)
    }

    /// Activate workflow in n8n
    pub async fn activate(&self, workflow_id: &str) -> Result<()> {
        let n8n_url = self
            .n8n_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("N8N_URL not configured"))?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("N8N_API_KEY not configured"))?;

        let url = format!("{}/api/v1/workflows/{}/activate", n8n_url, workflow_id);

        let response = self
            .client
            .patch(&url)
            .header("X-N8N-API-KEY", api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("n8n API error: {}", error_text));
        }

        log::info!("Workflow {} activated", workflow_id);

        Ok(())
    }

    /// Get workflow status from n8n
    pub async fn get_status(&self, workflow_id: &str) -> Result<WorkflowStatus> {
        let n8n_url = self
            .n8n_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("N8N_URL not configured"))?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("N8N_API_KEY not configured"))?;

        let url = format!("{}/api/v1/workflows/{}", n8n_url, workflow_id);

        let response = self
            .client
            .get(&url)
            .header("X-N8N-API-KEY", api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("n8n API error: {}", error_text));
        }

        let result: serde_json::Value = response.json().await?;

        Ok(WorkflowStatus {
            id: workflow_id.to_string(),
            name: result["name"].as_str().unwrap_or("Unknown").to_string(),
            active: result["active"].as_bool().unwrap_or(false),
            created_at: result["createdAt"].as_str().unwrap_or("").to_string(),
            updated_at: result["updatedAt"].as_str().unwrap_or("").to_string(),
        })
    }

    /// List all workflows
    pub async fn list_workflows(&self) -> Vec<Workflow> {
        self.workflows.read().await.clone()
    }

    /// Convert UserAction to WorkflowNode
    fn action_to_node(
        &self,
        action: &crate::jarvis::engines::pattern_detector::UserAction,
        index: usize,
    ) -> Result<WorkflowNode> {
        let node_id = format!("action_{}", index);

        // Map action types to n8n node types
        let (node_type, parameters) = match action.action_type.as_str() {
            "click" => (
                "n8n-nodes-base.executeCommand".to_string(),
                serde_json::json!({
                    "command": format!("click at {}", action.target)
                }),
            ),
            "type_text" => (
                "n8n-nodes-base.executeCommand".to_string(),
                serde_json::json!({
                    "command": format!("type '{}'", action.target)
                }),
            ),
            "open_app" => (
                "n8n-nodes-base.executeCommand".to_string(),
                serde_json::json!({
                    "command": format!("open {}", action.target)
                }),
            ),
            _ => (
                "n8n-nodes-base.function".to_string(),
                serde_json::json!({
                    "functionCode": format!("// Action: {}\n// Target: {}", action.action_type, action.target)
                }),
            ),
        };

        Ok(WorkflowNode {
            id: node_id.clone(),
            name: format!("{} - {}", action.action_type, action.target),
            node_type,
            type_version: 1,
            position: (100 + (index as i32 * 200), 300),
            parameters,
        })
    }

    /// Create trigger node
    fn create_trigger_node(&self, config: &TriggerConfig) -> Result<WorkflowNode> {
        let (node_type, parameters) = match config.trigger_type.as_str() {
            "manual" => (
                "n8n-nodes-base.manualTrigger".to_string(),
                serde_json::json!({}),
            ),
            "schedule" => (
                "n8n-nodes-base.scheduleTrigger".to_string(),
                serde_json::json!({
                    "rule": {
                        "interval": config.config.get("interval").cloned().unwrap_or_else(|| "1".to_string()),
                        "intervalSize": config.config.get("unit").cloned().unwrap_or_else(|| "hours".to_string())
                    }
                }),
            ),
            "webhook" => (
                "n8n-nodes-base.webhook".to_string(),
                serde_json::json!({
                    "path": config.config.get("path").cloned().unwrap_or_else(|| "webhook".to_string()),
                    "httpMethod": "POST"
                }),
            ),
            _ => (
                "n8n-nodes-base.manualTrigger".to_string(),
                serde_json::json!({}),
            ),
        };

        Ok(WorkflowNode {
            id: "trigger".to_string(),
            name: "Trigger".to_string(),
            node_type,
            type_version: 1,
            position: (100, 100),
            parameters,
        })
    }

    /// Create action node
    fn create_action_node(&self, action: &ActionConfig, index: usize) -> Result<WorkflowNode> {
        let node_id = format!("action_{}", index);

        let parameters = serde_json::to_value(&action.parameters)?;

        Ok(WorkflowNode {
            id: node_id.clone(),
            name: format!("{} - {}", action.action_type, action.target),
            node_type: "n8n-nodes-base.executeCommand".to_string(),
            type_version: 1,
            position: (100 + (index as i32 * 200), 300),
            parameters,
        })
    }

    /// Generate workflow suggestion text
    pub fn generate_suggestion(&self, workflow: &Workflow) -> String {
        format!(
            "Workflow '{}' with {} steps can automate this task. Would you like to deploy it?",
            workflow.name,
            workflow.nodes.len()
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStatus {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_builder_creation() {
        let builder = WorkflowBuilder::new().await.unwrap();
        let workflows = builder.list_workflows().await;
        assert_eq!(workflows.len(), 0);
    }

    #[tokio::test]
    async fn test_create_from_template() {
        let builder = WorkflowBuilder::new().await.unwrap();

        let template = WorkflowTemplate {
            template_type: TemplateType::Sequential,
            trigger: TriggerConfig {
                trigger_type: "manual".to_string(),
                config: HashMap::new(),
            },
            actions: vec![
                ActionConfig {
                    action_type: "click".to_string(),
                    target: "button".to_string(),
                    parameters: HashMap::new(),
                },
                ActionConfig {
                    action_type: "type_text".to_string(),
                    target: "input".to_string(),
                    parameters: HashMap::new(),
                },
            ],
        };

        let workflow = builder.create_from_template(template).await.unwrap();

        assert_eq!(workflow.nodes.len(), 3); // trigger + 2 actions
        assert!(!workflow.active);
        assert_eq!(workflow.tags[0], "sequential");
    }

    #[tokio::test]
    async fn test_workflow_suggestion() {
        let builder = WorkflowBuilder::new().await.unwrap();

        let workflow = Workflow {
            id: None,
            name: "Test Workflow".to_string(),
            nodes: vec![],
            connections: HashMap::new(),
            active: false,
            tags: vec![],
        };

        let suggestion = builder.generate_suggestion(&workflow);
        assert!(suggestion.contains("Test Workflow"));
        assert!(suggestion.contains("deploy"));
    }

    #[test]
    fn test_trigger_node_creation() {
        let builder = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(WorkflowBuilder::new())
            .unwrap();

        let config = TriggerConfig {
            trigger_type: "schedule".to_string(),
            config: {
                let mut map = HashMap::new();
                map.insert("interval".to_string(), "5".to_string());
                map.insert("unit".to_string(), "minutes".to_string());
                map
            },
        };

        let node = builder.create_trigger_node(&config).unwrap();

        assert_eq!(node.id, "trigger");
        assert_eq!(node.node_type, "n8n-nodes-base.scheduleTrigger");
    }

    #[test]
    fn test_action_node_creation() {
        let builder = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(WorkflowBuilder::new())
            .unwrap();

        let action = ActionConfig {
            action_type: "execute".to_string(),
            target: "script.sh".to_string(),
            parameters: HashMap::new(),
        };

        let node = builder.create_action_node(&action, 0).unwrap();

        assert_eq!(node.id, "action_0");
        assert!(node.name.contains("execute"));
        assert!(node.name.contains("script.sh"));
    }
}
