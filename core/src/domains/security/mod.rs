// Security Domain - Security and privacy controls
//
// Provides security checks, privacy controls, and data protection
//
// Components:
// - Security: Security validation and threat detection
// - Privacy: Privacy controls and data sanitization
// - GDPR: Data subject rights implementation (Articles 15, 17, 20)

pub mod security;
pub mod privacy;
pub mod gdpr;

pub use security::*;
pub use privacy::*;
pub use gdpr::*;
