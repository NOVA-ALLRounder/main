use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

use super::types::{JsonRpcRequest, JsonRpcResponse};
use super::{McpPrompt, McpResource, McpServer, McpTool};

pub struct McpClient {
    server: McpServer,
    process: Option<Child>,
    request_id: u64,
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
    pub prompts: Vec<McpPrompt>,
}

impl McpClient {
    pub fn new(server: McpServer) -> Self {
        Self {
            server,
            process: None,
            request_id: 0,
            tools: Vec::new(),
            resources: Vec::new(),
            prompts: Vec::new(),
        }
    }

    pub fn connect(&mut self) -> Result<()> {
        println!("🔌 [MCP] Connecting to: {}", self.server.name);

        let mut cmd = Command::new(&self.server.command);
        cmd.args(&self.server.args);
        for (key, value) in &self.server.env {
            cmd.env(key, value);
        }
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let child = cmd.spawn().context(format!(
            "Failed to start MCP server: {}",
            self.server.command
        ))?;

        self.process = Some(child);
        self.initialize()?;
        self.discover()?;

        println!(
            "✅ [MCP] Connected: {} tools, {} resources",
            self.tools.len(),
            self.resources.len()
        );

        Ok(())
    }

    fn send_request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let child = self
            .process
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("MCP server not connected"))?;

        self.request_id += 1;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: self.request_id,
            method: method.to_string(),
            params,
        };

        let request_json = serde_json::to_string(&request)?;

        if let Some(stdin) = child.stdin.as_mut() {
            writeln!(stdin, "{}", request_json)?;
            stdin.flush()?;
        }

        if let Some(stdout) = child.stdout.as_mut() {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            reader.read_line(&mut line)?;

            let response: JsonRpcResponse = serde_json::from_str(&line)?;
            if let Some(error) = response.error {
                return Err(anyhow::anyhow!(
                    "MCP error {}: {}",
                    error.code,
                    error.message
                ));
            }

            Ok(response.result.unwrap_or(json!(null)))
        } else {
            Err(anyhow::anyhow!("No stdout available"))
        }
    }

    fn initialize(&mut self) -> Result<()> {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {},
                "prompts": {}
            },
            "clientInfo": {
                "name": "steer-agent",
                "version": "0.1.0"
            }
        });

        let _result = self.send_request("initialize", Some(params))?;
        let _ = self.send_request("notifications/initialized", None);
        Ok(())
    }

    fn discover(&mut self) -> Result<()> {
        if let Ok(result) = self.send_request("tools/list", None) {
            if let Some(tools) = result["tools"].as_array() {
                self.tools = tools
                    .iter()
                    .filter_map(|t| serde_json::from_value(t.clone()).ok())
                    .collect();
            }
        }

        if let Ok(result) = self.send_request("resources/list", None) {
            if let Some(resources) = result["resources"].as_array() {
                self.resources = resources
                    .iter()
                    .filter_map(|r| serde_json::from_value(r.clone()).ok())
                    .collect();
            }
        }

        if let Ok(result) = self.send_request("prompts/list", None) {
            if let Some(prompts) = result["prompts"].as_array() {
                self.prompts = prompts
                    .iter()
                    .filter_map(|p| serde_json::from_value(p.clone()).ok())
                    .collect();
            }
        }

        Ok(())
    }

    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value> {
        println!("🔧 [MCP] Calling tool: {} with {:?}", name, arguments);
        let params = json!({ "name": name, "arguments": arguments });
        self.send_request("tools/call", Some(params))
    }

    pub fn read_resource(&mut self, uri: &str) -> Result<String> {
        println!("📖 [MCP] Reading resource: {}", uri);
        let params = json!({ "uri": uri });
        let result = self.send_request("resources/read", Some(params))?;

        if let Some(contents) = result["contents"].as_array() {
            if let Some(first) = contents.first() {
                if let Some(text) = first["text"].as_str() {
                    return Ok(text.to_string());
                }
            }
        }

        Ok(result.to_string())
    }

    pub fn get_prompt(&mut self, name: &str, arguments: Option<Value>) -> Result<String> {
        let params = json!({
            "name": name,
            "arguments": arguments.unwrap_or(json!({}))
        });
        let result = self.send_request("prompts/get", Some(params))?;

        if let Some(messages) = result["messages"].as_array() {
            let texts: Vec<String> = messages
                .iter()
                .filter_map(|m| m["content"]["text"].as_str().map(|s| s.to_string()))
                .collect();
            return Ok(texts.join("\n"));
        }

        Ok(result.to_string())
    }

    pub fn disconnect(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
            println!("🔌 [MCP] Disconnected: {}", self.server.name);
        }
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}
