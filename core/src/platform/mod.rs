pub mod apps;
pub mod macos;
mod registry;
pub mod types;
pub mod windows;

pub use apps::{
    app_matches_role, app_role_aliases, app_role_primary_name,
    parse_opened_or_switched_app_history_entry,
};
pub use registry::{current_platform, PlatformAdapter};
pub use types::{
    AppRole, BrowserSnapshotCapture, BrowserSnapshotSource, PlatformFixAction, PlatformKind,
    SystemSettingsTarget, UiAutomationProbe, UiBounds, UiSnapshotElement,
};
