use super::types::{AppRole, PlatformKind};

const MACOS_FILE_MANAGER_ALIASES: &[&str] = &["Finder", "finder", "파인더"];
const MACOS_MAIL_CLIENT_ALIASES: &[&str] = &[
    "Mail",
    "mail",
    "email",
    "메일",
    "이메일",
    "correo",
    "メール",
    "邮箱",
];
const MACOS_NOTES_APP_ALIASES: &[&str] = &[
    "Notes",
    "notes",
    "note",
    "Notion",
    "notion",
    "메모",
    "노트",
    "노션",
    "메모장",
    "notas",
    "nota",
    "メモ",
    "ノート",
    "笔记",
];
const MACOS_TEXT_EDITOR_ALIASES: &[&str] = &["TextEdit", "textedit", "텍스트에디트", "text editor"];
const MACOS_BROWSER_ALIASES: &[&str] = &[
    "Google Chrome",
    "chrome",
    "google chrome",
    "크롬",
    "Safari",
    "safari",
    "Arc",
    "browser",
];
const MACOS_CALENDAR_ALIASES: &[&str] = &[
    "Calendar",
    "calendar",
    "캘린더",
    "calendario",
    "カレンダー",
    "日历",
];
const MACOS_CALCULATOR_ALIASES: &[&str] = &[
    "Calculator",
    "calculator",
    "계산기",
    "calculadora",
    "計算機",
    "计算器",
];
const MACOS_PREVIEW_ALIASES: &[&str] = &["Preview", "preview", "미리보기"];

const WINDOWS_FILE_MANAGER_ALIASES: &[&str] =
    &["File Explorer", "Explorer", "explorer.exe", "file manager"];
const WINDOWS_MAIL_CLIENT_ALIASES: &[&str] =
    &["Outlook", "outlook.exe", "mail", "email", "메일", "이메일"];
const WINDOWS_NOTES_APP_ALIASES: &[&str] = &[
    "Notepad",
    "notepad.exe",
    "notes",
    "note",
    "Notion",
    "notion",
    "메모",
    "노트",
    "노션",
];
const WINDOWS_TEXT_EDITOR_ALIASES: &[&str] = &[
    "Notepad",
    "notepad.exe",
    "text editor",
    "메모장",
    "텍스트에디트",
];
const WINDOWS_BROWSER_ALIASES: &[&str] = &[
    "Microsoft Edge",
    "msedge.exe",
    "Google Chrome",
    "chrome.exe",
    "chrome",
    "browser",
];
const WINDOWS_CALENDAR_ALIASES: &[&str] = &["Calendar", "calendar", "Outlook Calendar", "캘린더"];
const WINDOWS_CALCULATOR_ALIASES: &[&str] = &["Calculator", "calculator", "calc.exe", "계산기"];
const WINDOWS_PREVIEW_ALIASES: &[&str] = &[
    "Photos",
    "photos.exe",
    "Photo Viewer",
    "preview",
    "미리보기",
];

const GENERIC_FILE_MANAGER_ALIASES: &[&str] = &["file manager"];
const GENERIC_MAIL_CLIENT_ALIASES: &[&str] = &["mail client"];
const GENERIC_NOTES_APP_ALIASES: &[&str] = &["notes app", "notion"];
const GENERIC_TEXT_EDITOR_ALIASES: &[&str] = &["text editor"];
const GENERIC_BROWSER_ALIASES: &[&str] = &["browser"];
const GENERIC_CALENDAR_ALIASES: &[&str] = &["calendar"];
const GENERIC_CALCULATOR_ALIASES: &[&str] = &["calculator"];
const GENERIC_PREVIEW_ALIASES: &[&str] = &["preview"];

pub fn app_role_aliases(kind: PlatformKind, role: AppRole) -> &'static [&'static str] {
    match (kind, role) {
        (PlatformKind::MacOS, AppRole::FileManager) => MACOS_FILE_MANAGER_ALIASES,
        (PlatformKind::MacOS, AppRole::MailClient) => MACOS_MAIL_CLIENT_ALIASES,
        (PlatformKind::MacOS, AppRole::NotesApp) => MACOS_NOTES_APP_ALIASES,
        (PlatformKind::MacOS, AppRole::TextEditor) => MACOS_TEXT_EDITOR_ALIASES,
        (PlatformKind::MacOS, AppRole::Browser) => MACOS_BROWSER_ALIASES,
        (PlatformKind::MacOS, AppRole::Calendar) => MACOS_CALENDAR_ALIASES,
        (PlatformKind::MacOS, AppRole::Calculator) => MACOS_CALCULATOR_ALIASES,
        (PlatformKind::MacOS, AppRole::Preview) => MACOS_PREVIEW_ALIASES,
        (PlatformKind::Windows, AppRole::FileManager) => WINDOWS_FILE_MANAGER_ALIASES,
        (PlatformKind::Windows, AppRole::MailClient) => WINDOWS_MAIL_CLIENT_ALIASES,
        (PlatformKind::Windows, AppRole::NotesApp) => WINDOWS_NOTES_APP_ALIASES,
        (PlatformKind::Windows, AppRole::TextEditor) => WINDOWS_TEXT_EDITOR_ALIASES,
        (PlatformKind::Windows, AppRole::Browser) => WINDOWS_BROWSER_ALIASES,
        (PlatformKind::Windows, AppRole::Calendar) => WINDOWS_CALENDAR_ALIASES,
        (PlatformKind::Windows, AppRole::Calculator) => WINDOWS_CALCULATOR_ALIASES,
        (PlatformKind::Windows, AppRole::Preview) => WINDOWS_PREVIEW_ALIASES,
        (_, AppRole::FileManager) => GENERIC_FILE_MANAGER_ALIASES,
        (_, AppRole::MailClient) => GENERIC_MAIL_CLIENT_ALIASES,
        (_, AppRole::NotesApp) => GENERIC_NOTES_APP_ALIASES,
        (_, AppRole::TextEditor) => GENERIC_TEXT_EDITOR_ALIASES,
        (_, AppRole::Browser) => GENERIC_BROWSER_ALIASES,
        (_, AppRole::Calendar) => GENERIC_CALENDAR_ALIASES,
        (_, AppRole::Calculator) => GENERIC_CALCULATOR_ALIASES,
        (_, AppRole::Preview) => GENERIC_PREVIEW_ALIASES,
    }
}

pub fn app_role_primary_name(kind: PlatformKind, role: AppRole) -> &'static str {
    app_role_aliases(kind, role)[0]
}

pub fn app_matches_role(kind: PlatformKind, role: AppRole, value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    app_role_aliases(kind, role)
        .iter()
        .any(|candidate| candidate.to_ascii_lowercase() == normalized)
}

pub fn parse_opened_or_switched_app_history_entry(entry: &str) -> Option<&str> {
    entry
        .strip_prefix("Opened app: ")
        .or_else(|| entry.strip_prefix("opened app: "))
        .or_else(|| entry.strip_prefix("Switched to app: "))
        .or_else(|| entry.strip_prefix("switched to app: "))
        .map(str::trim)
        .filter(|app| !app.is_empty())
}
