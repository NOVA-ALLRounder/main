// Workflow Domain - Workflow orchestration and automation
//
// Provides workflow management, command queuing, and dynamic control
//
// Components:
// - WorkflowSchema: Workflow definition and validation
// - CommandQueue: Command queuing and scheduling
// - DynamicController: Runtime control and adaptation
// - SlotFiller: Parameter filling and context binding

pub mod workflow_schema;
pub mod command_queue;
pub mod dynamic_controller;
pub mod slot_filler;
pub mod replanning_config;
pub mod replan_templates;

pub use workflow_schema::*;
pub use command_queue::*;
pub use dynamic_controller::*;
pub use slot_filler::*;
pub use replanning_config::*;
pub use replan_templates::*;
