// Project Infrastructure - Project analysis and scanning
//
// Provides project structure analysis and dependency management
//
// Components:
// - ProjectScanner: Project structure scanning
// - DependencyCheck: Dependency validation
// - StaticChecks: Static code analysis

pub mod project_scanner;
pub mod dependency_check;
pub mod static_checks;

pub use project_scanner::*;
pub use dependency_check::*;
pub use static_checks::*;
