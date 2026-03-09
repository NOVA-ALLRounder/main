use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;

use super::{McpClient, McpServer, McpTool};

pub struct McpRegistry {
    clients: HashMap<String, McpClient>,
    config_path: std::path::PathBuf,
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl McpRegistry {
    pub fn new() -> Self {
        let config_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".steer")
            .join("mcp_servers.json");

        Self {
            clients: HashMap::new(),
            config_path,
        }
    }

    pub fn load_config(&mut self) -> Result<()> {
        if !self.config_path.exists() {
            self.create_default_config()?;
        }

        let content = std::fs::read_to_string(&self.config_path)?;
        let servers: HashMap<String, McpServer> = serde_json::from_str(&content)?;

        for (name, mut server) in servers {
            server.name = name.clone();
            if server.enabled {
                let mut client = McpClient::new(server);
                if client.connect().is_ok() {
                    self.clients.insert(name, client);
                }
            }
        }

        Ok(())
    }

    fn create_default_config(&self) -> Result<()> {
        let default = json!({
            "filesystem": {
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-filesystem", "/Users"],
                "env": {},
                "enabled": false
            },
            "memory": {
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-memory"],
                "env": {},
                "enabled": false
            },
            "brave-search": {
                "command": "npx",
                "args": ["-y", "@anthropic-ai/claude-mcp-server-brave"],
                "env": {"BRAVE_API_KEY": ""},
                "enabled": false
            },
            "google-calendar": {
                "command": "npx",
                "args": ["-y", "@anthropic-ai/claude-mcp-server-google-calendar"],
                "env": {},
                "enabled": false
            },
            "slack": {
                "command": "npx",
                "args": ["-y", "@anthropic-ai/claude-mcp-server-slack"],
                "env": {"SLACK_BOT_TOKEN": ""},
                "enabled": false
            }
        });

        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&self.config_path, serde_json::to_string_pretty(&default)?)?;
        println!(
            "📝 [MCP] Created default config: {}",
            self.config_path.display()
        );
        Ok(())
    }

    pub fn get(&mut self, name: &str) -> Option<&mut McpClient> {
        self.clients.get_mut(name)
    }

    pub fn list_all_tools(&self) -> Vec<(String, McpTool)> {
        self.clients
            .iter()
            .flat_map(|(server_name, client)| {
                client
                    .tools
                    .iter()
                    .map(move |tool| (server_name.clone(), tool.clone()))
            })
            .collect()
    }

    pub fn call_tool(&mut self, server: &str, tool: &str, args: Value) -> Result<Value> {
        let client = self
            .clients
            .get_mut(server)
            .ok_or_else(|| anyhow::anyhow!("MCP server '{}' not found", server))?;
        client.call_tool(tool, args)
    }

    pub fn add_server(&mut self, server: McpServer) -> Result<()> {
        let name = server.name.clone();
        let mut client = McpClient::new(server);
        client.connect()?;
        self.clients.insert(name, client);
        Ok(())
    }
}

lazy_static::lazy_static! {
    static ref MCP_REGISTRY: std::sync::Mutex<Option<McpRegistry>> = std::sync::Mutex::new(None);
}

pub fn init_mcp() -> Result<()> {
    let mut guard = MCP_REGISTRY
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

    if guard.is_none() {
        let mut registry = McpRegistry::new();
        if let Err(e) = registry.load_config() {
            eprintln!("❌ [MCP] Failed to load config: {}", e);
            eprintln!("    Path was: {}", registry.config_path.display());
        }
        *guard = Some(registry);
    }

    Ok(())
}

pub fn get_mcp_registry() -> Result<std::sync::MutexGuard<'static, Option<McpRegistry>>> {
    MCP_REGISTRY
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock error: {}", e))
}

pub fn call_mcp_tool(server: &str, tool: &str, args: Value) -> Result<Value> {
    let mut guard = get_mcp_registry()?;
    let registry = guard
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("MCP not initialized"))?;
    registry.call_tool(server, tool, args)
}
