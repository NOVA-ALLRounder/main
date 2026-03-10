use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformKind {
    MacOS,
    Windows,
    Linux,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppRole {
    FileManager,
    MailClient,
    NotesApp,
    TextEditor,
    Browser,
    Calendar,
    Calculator,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemSettingsTarget {
    UiAutomation,
    ScreenCapture,
    InputMonitoring,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformFixAction {
    ActivateApp(AppRole),
    PrepareIsolatedMode,
    OpenSystemSettings(SystemSettingsTarget),
    RequestUiAutomationAccess,
    RequestScreenCaptureAccess,
    RevealPath(PathBuf),
    FillDefaultMailRecipient { recipient: String },
    CleanupOutgoingMailDrafts,
    SaveFrontTextDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAutomationProbe {
    pub app_name: String,
    pub window_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSnapshotElement {
    pub role: String,
    pub name: String,
    pub bounds: Option<UiBounds>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserSnapshotSource {
    Accessibility,
    Peekaboo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserSnapshotCapture {
    pub elements: Vec<UiSnapshotElement>,
    pub source: BrowserSnapshotSource,
    pub snapshot_id: Option<String>,
}
