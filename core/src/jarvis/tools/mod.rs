// JARVIS Tool System (Clawdbot pattern)
//
// DEPRECATED: This entire module is deprecated as of v0.2.0 and will be removed in v0.4.0
// Please migrate to the Skills system (jarvis::skills)
// See docs/MIGRATION_GUIDE.md for detailed migration instructions
//
// Replacement modules:
// - Tool ??Skill (jarvis::skills::Skill)
// - ToolRegistry ??SkillRegistry (jarvis::skills::SkillRegistry)
// - ToolParams ??SkillContext (jarvis::skills::SkillContext)
// - ToolResult ??SkillResult (jarvis::skills::SkillResult)

#![allow(deprecated)]

pub mod excel_tool;
pub mod memory_tool;
pub mod n8n_tool;
pub mod telegram_tool;
pub mod tool_policy;
pub mod tool_registry;
pub mod tool_trait;
pub mod vision_tool;
pub mod windows_tool;

pub use tool_policy::{ToolPolicy, ToolPolicyManager};
pub use tool_registry::ToolRegistry;
pub use tool_trait::{Tool, ToolParams, ToolResult};

// Tool implementations
pub use excel_tool::ExcelTool;
pub use memory_tool::MemoryTool;
pub use n8n_tool::N8nTool;
pub use telegram_tool::TelegramTool;
pub use vision_tool::VisionTool;
pub use windows_tool::WindowsTool;
