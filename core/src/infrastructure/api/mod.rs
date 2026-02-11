// API Infrastructure - REST API server
//
// Current: Re-exports for backward compatibility
// Future: Gradual migration to modular structure
//
// Planned structure:
// - server.rs: Main server setup (Axum app builder)
// - routes/
//   - chat.rs: Chat endpoints
//   - jarvis.rs: JARVIS endpoints
//   - verification.rs: Verification endpoints
//   - workflow.rs: Workflow/automation endpoints
//   - health.rs: Health check endpoints
// - handlers/
//   - chat_handler.rs: Chat request handlers
//   - jarvis_handler.rs: JARVIS request handlers
//   - verification_handler.rs: Verification handlers
// - middleware/
//   - auth.rs: Authentication middleware
//   - logging.rs: Request logging
//   - cors.rs: CORS configuration
//
// Migration strategy: Extract route groups one at a time while maintaining
// backward compatibility

// Local module
pub mod api_server;

// Re-export for backward compatibility
pub use api_server::*;
