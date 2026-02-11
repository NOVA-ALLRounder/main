// Unified Execution Framework
//
// Provides a trait-based system for different execution engines

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub message: String,
    pub output: Option<String>,
    pub error: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ExecutionResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            output: None,
            error: None,
            metadata: HashMap::new(),
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            message: "Execution failed".to_string(),
            output: None,
            error: Some(error.into()),
            metadata: HashMap::new(),
        }
    }

    pub fn with_output(mut self, output: impl Into<String>) -> Self {
        self.output = Some(output.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Execution context
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub workdir: Option<String>,
    pub timeout_ms: Option<u64>,
    pub dry_run: bool,
    pub custom_params: HashMap<String, String>,
}

/// Engine types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineType {
    UI,
    Browser,
    Shell,
}

/// Trait for all execution engines
#[async_trait::async_trait]
pub trait ExecutionEngine: Send + Sync {
    /// Name of the execution engine
    fn name(&self) -> &str;

    /// Type of engine
    fn engine_type(&self) -> EngineType;

    /// Description of what this engine does
    fn description(&self) -> &str;

    /// Execute a command
    async fn execute(&self, command: &str, context: &ExecutionContext) -> Result<ExecutionResult>;

    /// Check if this engine can handle the given command
    fn can_handle(&self, command: &str) -> bool;

    /// Whether this engine is available in the current environment
    fn is_available(&self) -> bool {
        true
    }
}

/// Registry for execution engines
pub struct ExecutionRegistry {
    engines: HashMap<EngineType, Arc<dyn ExecutionEngine>>,
}

impl ExecutionRegistry {
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
        }
    }

    /// Register an execution engine
    pub fn register(&mut self, engine: Arc<dyn ExecutionEngine>) {
        self.engines.insert(engine.engine_type(), engine);
    }

    /// Get engine by type
    pub fn get_engine(&self, engine_type: EngineType) -> Option<Arc<dyn ExecutionEngine>> {
        self.engines.get(&engine_type).cloned()
    }

    /// Execute command using the appropriate engine
    pub async fn execute(
        &self,
        command: &str,
        engine_type: EngineType,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        if let Some(engine) = self.engines.get(&engine_type) {
            engine.execute(command, context).await
        } else {
            Ok(ExecutionResult::failure(format!(
                "Engine {:?} not registered",
                engine_type
            )))
        }
    }

    /// Find and execute using the first capable engine
    pub async fn execute_auto(
        &self,
        command: &str,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        for engine in self.engines.values() {
            if engine.can_handle(command) && engine.is_available() {
                return engine.execute(command, context).await;
            }
        }

        Ok(ExecutionResult::failure(
            "No capable engine found for command".to_string(),
        ))
    }

    /// Get all registered engines
    pub fn list_engines(&self) -> Vec<(EngineType, String)> {
        self.engines
            .iter()
            .map(|(t, e)| (*t, e.name().to_string()))
            .collect()
    }
}

impl Default for ExecutionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
