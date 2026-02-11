# Tool → Skill Migration Guide

## Overview

Steer is migrating from the legacy `ToolRegistry` system to the new `SkillRegistry` system for better modularity, extensibility, and maintainability.

## Timeline

- **v0.1.x**: Both systems coexist (current)
- **v0.2.0**: ToolRegistry marked as deprecated (this release)
- **v0.3.0**: ToolRegistry will show deprecation warnings
- **v0.4.0**: ToolRegistry will be removed entirely

## Why Migrate?

### Old System (Tools)
```rust
// Simple but inflexible
pub trait Tool {
    fn name(&self) -> &str;
    async fn execute(&self, params: ToolParams) -> Result<ToolResult>;
}
```

**Limitations:**
- No eligibility checking (can't detect if dependencies are available)
- No approval mechanism for risky actions
- No metadata about capabilities
- Hard to extend

### New System (Skills)
```rust
// Rich, extensible interface
pub trait Skill {
    fn metadata(&self) -> SkillMetadata;
    fn check_eligibility(&self) -> EligibilityResult;
    async fn execute(&self, ctx: SkillContext) -> SkillResult;
    fn requires_approval(&self, action: &str) -> bool;
}
```

**Benefits:**
- ✅ Runtime eligibility checks (e.g., check if Telegram token exists)
- ✅ Fine-grained approval controls
- ✅ Rich metadata for discovery
- ✅ Better error handling
- ✅ Cleanup hooks

## Migration Steps

### For Tool Developers

#### Before (Tool):
```rust
use async_trait::async_trait;
use crate::jarvis::tools::{Tool, ToolParams, ToolResult};

pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str {
        "my_tool"
    }

    fn description(&self) -> &str {
        "Does something useful"
    }

    async fn execute(&self, params: ToolParams) -> Result<ToolResult> {
        // Implementation
        Ok(ToolResult::success("Done"))
    }
}
```

#### After (Skill):
```rust
use async_trait::async_trait;
use crate::jarvis::skills::{Skill, SkillContext, SkillResult, SkillMetadata, EligibilityResult};

pub struct MySkill;

#[async_trait]
impl Skill for MySkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "my_skill".to_string(),
            description: "Does something useful".to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                ActionMetadata {
                    name: "do_something".to_string(),
                    description: "Performs the action".to_string(),
                    parameters: vec![
                        ParameterMetadata {
                            name: "input".to_string(),
                            description: "Input parameter".to_string(),
                            required: true,
                            param_type: "string".to_string(),
                        }
                    ],
                    examples: vec!["my_skill do_something --input=test".to_string()],
                }
            ],
            requirements: vec![],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        // Check if dependencies are available
        EligibilityResult::eligible()
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        // Implementation with rich context
        SkillResult::success("Done")
    }

    fn requires_approval(&self, action: &str) -> bool {
        // Override for risky actions
        false
    }
}
```

### For Application Code

#### Before:
```rust
// Register tools
let tool_registry = Arc::new(RwLock::new(ToolRegistry::new()));
{
    let mut registry = tool_registry.write().await;
    registry.register(Arc::new(MyTool::new()));
}

// Execute
let result = tool_registry.read().await
    .execute("my_tool", ToolParams::new("action"))
    .await?;
```

#### After:
```rust
// Register skills
let skill_registry = Arc::new(SkillRegistry::new());
skill_registry.register(Arc::new(MySkill::new())).await;

// Execute
let ctx = SkillContext {
    session_key: "session_123".to_string(),
    action: "do_something".to_string(),
    params: HashMap::new(),
    user_context: UserContext::default(),
};

let result = skill_registry.execute("my_skill", ctx).await;
```

## Deprecated APIs

The following types and functions are deprecated:

### Deprecated (will be removed in v0.4.0)
- `jarvis::tools::ToolRegistry`
- `jarvis::tools::Tool`
- `jarvis::tools::ToolParams`
- `jarvis::tools::ToolResult`
- `jarvis::command_parser::CommandParser` (will be rewritten)

### Replacement
- Use `jarvis::skills::SkillRegistry`
- Use `jarvis::skills::Skill`
- Use `jarvis::skills::SkillContext`
- Use `jarvis::skills::SkillResult`

## Migration Checklist

- [ ] Review your tool implementations
- [ ] Create equivalent skill implementations
- [ ] Add eligibility checks
- [ ] Add approval requirements for risky actions
- [ ] Update registration code
- [ ] Update execution code
- [ ] Update tests
- [ ] Remove tool code after verification

## Example: Complete Migration

See [examples/tool_to_skill_migration.rs](../examples/tool_to_skill_migration.rs) for a complete example.

## FAQ

### Q: Can I use both systems during migration?
**A:** Yes! Both systems coexist in v0.2.x. However, we recommend migrating as soon as possible.

### Q: What if my tool needs special cleanup?
**A:** Implement the `cleanup()` method in your skill:
```rust
async fn cleanup(&self) -> Result<()> {
    // Close connections, free resources, etc.
    Ok(())
}
```

### Q: How do I handle async initialization?
**A:** Check dependencies in `check_eligibility()`:
```rust
fn check_eligibility(&self) -> EligibilityResult {
    if std::env::var("MY_API_KEY").is_err() {
        return EligibilityResult::not_eligible("MY_API_KEY not set");
    }
    EligibilityResult::eligible()
}
```

### Q: Can skills depend on other skills?
**A:** Not directly. Use the orchestrator to chain skills together.

## Support

For questions or issues during migration:
- Open an issue on GitHub
- Check the [Skills Documentation](SKILLS.md)
- Review [Architecture Documentation](ARCHITECTURE.md)

---

**Last Updated:** 2026-02-06
**Version:** 0.2.0
