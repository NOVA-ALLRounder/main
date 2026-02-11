// Monitoring Domain - System observability and feedback collection
//
// Provides monitoring, logging, and feedback mechanisms
//
// Components:
// - Monitor: System monitoring and health checks
// - FeedbackCollector: User feedback and telemetry
// - Notifier: Event notifications and alerts

pub mod monitor;
pub mod feedback_collector;
pub mod notifier;

pub use monitor::*;
pub use feedback_collector::*;
pub use notifier::*;
