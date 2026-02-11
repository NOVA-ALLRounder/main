// Database Infrastructure - Database access and queries
//
// Current: Re-exports for backward compatibility
// Future: Gradual migration to modular structure
//
// Planned structure:
// - connection.rs: Database connection management
// - schema.rs: Database schemas (already separate)
// - queries/
//   - sessions.rs: Session-related queries
//   - routines.rs: Routine management queries
//   - recommendations.rs: Recommendation queries
//   - patterns.rs: Pattern storage queries
//   - events.rs: Event log queries
// - transactions.rs: Transaction utilities
//
// Migration strategy: Extract query groups one at a time while maintaining
// backward compatibility through re-exports

// Local modules
pub mod db;
pub mod schema;

// Re-export for backward compatibility
pub use db::*;
pub use schema::*;
