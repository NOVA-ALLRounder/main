# JARVIS Skills Documentation

This document provides comprehensive documentation for all implemented skills in the JARVIS system.

## Table of Contents

- [Overview](#overview)
- [ComputerUseSkill](#computeruseskill)
- [EmailSkill](#emailskill)
- [TelegramSkill](#telegramskill)
- [Skill Development Guide](#skill-development-guide)

---

## Overview

JARVIS uses a modular skill system where each skill implements the `Skill` trait. Skills can be registered with the orchestrator and executed independently or as part of complex workflows.

### Skill Interface

All skills implement the following interface:

```rust
#[async_trait]
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn capabilities(&self) -> Vec<String>;
    fn can_handle(&self, task: &TaskStep) -> bool;
    async fn execute(&self, task: &TaskStep) -> Result<serde_json::Value, Box<dyn std::error::Error>>;
}
```

---

## ComputerUseSkill

The ComputerUseSkill provides comprehensive computer control capabilities including screen capture, keyboard/mouse control, and window management.

### Capabilities

- `screenshot` - Capture screen images
- `type_text` - Simulate keyboard typing
- `key_press` - Press specific keys
- `mouse_move` - Move mouse cursor
- `mouse_click` - Click mouse buttons
- `mouse_scroll` - Scroll mouse wheel
- `list_windows` - List open windows
- `focus_window` - Focus specific window
- `get_screen_size` - Get screen dimensions

### Setup Instructions

No special setup required. Works out of the box on Windows, macOS, and Linux.

**Platform Notes:**
- Windows: Uses Windows API for keyboard/mouse control
- macOS: Requires accessibility permissions
- Linux: Requires X11 or Wayland support

### Actions and Parameters

#### 1. Screenshot

Capture the screen or specific region.

**Parameters:**
- `display` (optional): Display number (default: 0)
- `region` (optional): Capture specific region `{"x": 0, "y": 0, "width": 1920, "height": 1080}`

**Example:**
```json
{
  "id": "screenshot_task",
  "action": "screenshot",
  "parameters": {
    "display": 0
  },
  "dependencies": []
}
```

**Returns:**
```json
{
  "success": true,
  "image_path": "/tmp/screenshot_20240206_123456.png",
  "width": 1920,
  "height": 1080
}
```

#### 2. Type Text

Simulate keyboard typing.

**Parameters:**
- `text` (required): Text to type
- `delay_ms` (optional): Delay between keystrokes in milliseconds (default: 10)

**Example:**
```json
{
  "id": "type_task",
  "action": "type_text",
  "parameters": {
    "text": "Hello, JARVIS!",
    "delay_ms": 50
  },
  "dependencies": []
}
```

#### 3. Key Press

Press specific keys or key combinations.

**Parameters:**
- `key` (required): Key to press (e.g., "enter", "ctrl", "alt", "a")
- `modifiers` (optional): Array of modifier keys `["ctrl", "shift"]`

**Example:**
```json
{
  "id": "key_press_task",
  "action": "key_press",
  "parameters": {
    "key": "c",
    "modifiers": ["ctrl"]
  },
  "dependencies": []
}
```

**Supported Keys:**
- Alphanumeric: `a-z`, `0-9`
- Special: `enter`, `escape`, `tab`, `space`, `backspace`
- Modifiers: `ctrl`, `alt`, `shift`, `meta`
- Function: `f1`-`f12`
- Navigation: `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`

#### 4. Mouse Move

Move mouse cursor to specific coordinates.

**Parameters:**
- `x` (required): X coordinate
- `y` (required): Y coordinate
- `smooth` (optional): Smooth movement (default: false)

**Example:**
```json
{
  "id": "mouse_move_task",
  "action": "mouse_move",
  "parameters": {
    "x": 500,
    "y": 300,
    "smooth": true
  },
  "dependencies": []
}
```

#### 5. Mouse Click

Click mouse button.

**Parameters:**
- `button` (optional): Button to click - "left", "right", "middle" (default: "left")
- `x` (optional): X coordinate (moves before clicking)
- `y` (optional): Y coordinate (moves before clicking)
- `double` (optional): Double click (default: false)

**Example:**
```json
{
  "id": "click_task",
  "action": "mouse_click",
  "parameters": {
    "x": 500,
    "y": 300,
    "button": "left",
    "double": false
  },
  "dependencies": []
}
```

#### 6. Mouse Scroll

Scroll mouse wheel.

**Parameters:**
- `direction` (required): "up" or "down"
- `amount` (optional): Scroll amount (default: 1)

**Example:**
```json
{
  "id": "scroll_task",
  "action": "mouse_scroll",
  "parameters": {
    "direction": "down",
    "amount": 3
  },
  "dependencies": []
}
```

#### 7. List Windows

List all open windows.

**Example:**
```json
{
  "id": "list_windows_task",
  "action": "list_windows",
  "parameters": {},
  "dependencies": []
}
```

**Returns:**
```json
{
  "windows": [
    {"id": "12345", "title": "Browser - Google Chrome", "app": "chrome.exe"},
    {"id": "67890", "title": "Document.txt - Notepad", "app": "notepad.exe"}
  ]
}
```

#### 8. Focus Window

Focus specific window.

**Parameters:**
- `window_id` (required): Window ID from list_windows
- `title` (optional): Window title (alternative to window_id)

**Example:**
```json
{
  "id": "focus_task",
  "action": "focus_window",
  "parameters": {
    "window_id": "12345"
  },
  "dependencies": ["list_windows_task"]
}
```

#### 9. Get Screen Size

Get screen dimensions.

**Example:**
```json
{
  "id": "screen_size_task",
  "action": "get_screen_size",
  "parameters": {},
  "dependencies": []
}
```

**Returns:**
```json
{
  "width": 1920,
  "height": 1080
}
```

### Error Handling

The skill handles errors gracefully and returns descriptive error messages:

- `ScreenshotError`: Failed to capture screen
- `KeyboardError`: Failed to simulate keyboard input
- `MouseError`: Failed to simulate mouse input
- `WindowError`: Failed to manipulate windows
- `InvalidParameter`: Invalid or missing parameters

---

## EmailSkill

The EmailSkill provides email functionality including sending emails via SMTP and reading emails via IMAP.

### Capabilities

- `send_email` - Send emails with attachments
- `read_emails` - Read emails from mailbox
- `search_emails` - Search emails by criteria
- `mark_read` - Mark email as read
- `mark_unread` - Mark email as unread
- `delete_email` - Delete email

### Setup Instructions

#### Environment Variables

The EmailSkill requires the following environment variables:

**For SMTP (Sending):**
```bash
SMTP_SERVER=smtp.gmail.com
SMTP_PORT=587
SMTP_USERNAME=your-email@gmail.com
SMTP_PASSWORD=your-app-password
SMTP_FROM=your-email@gmail.com
```

**For IMAP (Reading):**
```bash
IMAP_SERVER=imap.gmail.com
IMAP_PORT=993
IMAP_USERNAME=your-email@gmail.com
IMAP_PASSWORD=your-app-password
```

#### Gmail Setup

1. Enable 2-factor authentication in your Google account
2. Generate an App Password:
   - Go to Google Account > Security > 2-Step Verification > App passwords
   - Generate a new app password for "Mail"
   - Use this password in `SMTP_PASSWORD` and `IMAP_PASSWORD`

#### Other Email Providers

**Outlook/Office365:**
```bash
SMTP_SERVER=smtp.office365.com
SMTP_PORT=587
IMAP_SERVER=outlook.office365.com
IMAP_PORT=993
```

**Yahoo:**
```bash
SMTP_SERVER=smtp.mail.yahoo.com
SMTP_PORT=587
IMAP_SERVER=imap.mail.yahoo.com
IMAP_PORT=993
```

### Actions and Parameters

#### 1. Send Email

Send an email with optional attachments.

**Parameters:**
- `to` (required): Recipient email address (or array of addresses)
- `subject` (required): Email subject
- `body` (required): Email body (supports HTML)
- `cc` (optional): CC recipients (string or array)
- `bcc` (optional): BCC recipients (string or array)
- `attachments` (optional): Array of file paths
- `html` (optional): Send as HTML (default: false)

**Example:**
```json
{
  "id": "send_email_task",
  "action": "send_email",
  "parameters": {
    "to": ["recipient@example.com", "other@example.com"],
    "subject": "Daily Report",
    "body": "<h1>Report</h1><p>Status: Complete</p>",
    "html": true,
    "attachments": ["/path/to/report.pdf", "/path/to/chart.png"],
    "cc": ["manager@example.com"]
  },
  "dependencies": []
}
```

**Returns:**
```json
{
  "success": true,
  "message_id": "<abc123@example.com>",
  "sent_at": "2024-02-06T12:34:56Z"
}
```

#### 2. Read Emails

Read emails from mailbox.

**Parameters:**
- `folder` (optional): Mailbox folder (default: "INBOX")
- `limit` (optional): Maximum number of emails (default: 10)
- `unread_only` (optional): Only unread emails (default: false)
- `since_date` (optional): ISO 8601 date string

**Example:**
```json
{
  "id": "read_emails_task",
  "action": "read_emails",
  "parameters": {
    "folder": "INBOX",
    "limit": 5,
    "unread_only": true
  },
  "dependencies": []
}
```

**Returns:**
```json
{
  "emails": [
    {
      "id": "12345",
      "from": "sender@example.com",
      "subject": "Meeting Tomorrow",
      "date": "2024-02-06T10:30:00Z",
      "body": "Email body text...",
      "unread": true,
      "attachments": ["document.pdf"]
    }
  ],
  "count": 5
}
```

#### 3. Search Emails

Search emails by criteria.

**Parameters:**
- `query` (required): Search query
- `folder` (optional): Mailbox folder (default: "INBOX")
- `limit` (optional): Maximum results (default: 10)

**Example:**
```json
{
  "id": "search_task",
  "action": "search_emails",
  "parameters": {
    "query": "from:boss@company.com subject:urgent",
    "folder": "INBOX",
    "limit": 20
  },
  "dependencies": []
}
```

#### 4. Mark Read/Unread

Mark email as read or unread.

**Parameters:**
- `email_id` (required): Email ID from read_emails
- `folder` (optional): Mailbox folder (default: "INBOX")

**Example:**
```json
{
  "id": "mark_read_task",
  "action": "mark_read",
  "parameters": {
    "email_id": "12345"
  },
  "dependencies": ["read_emails_task"]
}
```

#### 5. Delete Email

Delete an email.

**Parameters:**
- `email_id` (required): Email ID from read_emails
- `folder` (optional): Mailbox folder (default: "INBOX")
- `permanent` (optional): Permanently delete (default: false, moves to trash)

**Example:**
```json
{
  "id": "delete_task",
  "action": "delete_email",
  "parameters": {
    "email_id": "12345",
    "permanent": false
  },
  "dependencies": []
}
```

### Error Handling

- `AuthenticationError`: Invalid credentials
- `ConnectionError`: Cannot connect to mail server
- `InvalidRecipient`: Invalid email address
- `AttachmentError`: Cannot attach file
- `MailboxError`: Cannot access mailbox/folder

---

## TelegramSkill

The TelegramSkill provides Telegram bot integration for sending messages, photos, documents, and managing chats.

### Capabilities

- `send_message` - Send text message
- `send_photo` - Send photo/image
- `send_document` - Send file/document
- `send_location` - Send location
- `get_updates` - Get new messages
- `get_chat_info` - Get chat information
- `edit_message` - Edit sent message
- `delete_message` - Delete message

### Setup Instructions

#### 1. Create Telegram Bot

1. Open Telegram and search for `@BotFather`
2. Send `/newbot` command
3. Follow instructions to create your bot
4. Copy the bot token (format: `123456789:ABCdefGHIjklMNOpqrsTUVwxyz`)

#### 2. Get Your Chat ID

**Method 1: Using @userinfobot**
1. Search for `@userinfobot` in Telegram
2. Start the bot
3. Your Chat ID will be displayed

**Method 2: Using API**
1. Send a message to your bot
2. Visit: `https://api.telegram.org/bot<YOUR_BOT_TOKEN>/getUpdates`
3. Find your `chat.id` in the response

#### 3. Set Environment Variable

```bash
TELEGRAM_BOT_TOKEN=123456789:ABCdefGHIjklMNOpqrsTUVwxyz
```

### Actions and Parameters

#### 1. Send Message

Send text message to chat.

**Parameters:**
- `chat_id` (required): Chat ID (string or number)
- `message` (required): Message text
- `parse_mode` (optional): "Markdown" or "HTML"
- `disable_notification` (optional): Silent message (default: false)

**Example:**
```json
{
  "id": "send_message_task",
  "action": "send_message",
  "parameters": {
    "chat_id": "123456789",
    "message": "**Hello from JARVIS!**\n\nTask completed successfully.",
    "parse_mode": "Markdown"
  },
  "dependencies": []
}
```

**Returns:**
```json
{
  "success": true,
  "message_id": 42,
  "chat_id": 123456789,
  "sent_at": "2024-02-06T12:34:56Z"
}
```

#### 2. Send Photo

Send photo/image to chat.

**Parameters:**
- `chat_id` (required): Chat ID
- `photo` (required): File path or URL
- `caption` (optional): Photo caption
- `parse_mode` (optional): "Markdown" or "HTML"

**Example:**
```json
{
  "id": "send_photo_task",
  "action": "send_photo",
  "parameters": {
    "chat_id": "123456789",
    "photo": "/tmp/screenshot.png",
    "caption": "Screenshot at 12:34"
  },
  "dependencies": ["screenshot_task"]
}
```

#### 3. Send Document

Send file/document to chat.

**Parameters:**
- `chat_id` (required): Chat ID
- `document` (required): File path
- `caption` (optional): Document caption
- `filename` (optional): Custom filename

**Example:**
```json
{
  "id": "send_doc_task",
  "action": "send_document",
  "parameters": {
    "chat_id": "123456789",
    "document": "/path/to/report.pdf",
    "caption": "Monthly Report",
    "filename": "february_report.pdf"
  },
  "dependencies": []
}
```

#### 4. Send Location

Send location to chat.

**Parameters:**
- `chat_id` (required): Chat ID
- `latitude` (required): Latitude coordinate
- `longitude` (required): Longitude coordinate
- `title` (optional): Location title

**Example:**
```json
{
  "id": "send_location_task",
  "action": "send_location",
  "parameters": {
    "chat_id": "123456789",
    "latitude": 37.7749,
    "longitude": -122.4194,
    "title": "San Francisco"
  },
  "dependencies": []
}
```

#### 5. Get Updates

Get new messages sent to bot.

**Parameters:**
- `limit` (optional): Maximum updates (default: 10)
- `timeout` (optional): Long polling timeout in seconds (default: 0)

**Example:**
```json
{
  "id": "get_updates_task",
  "action": "get_updates",
  "parameters": {
    "limit": 5
  },
  "dependencies": []
}
```

**Returns:**
```json
{
  "updates": [
    {
      "update_id": 123456,
      "message": {
        "message_id": 42,
        "from": {"id": 123456789, "username": "user123"},
        "chat": {"id": 123456789, "type": "private"},
        "date": 1707220496,
        "text": "Hello bot!"
      }
    }
  ]
}
```

#### 6. Edit Message

Edit previously sent message.

**Parameters:**
- `chat_id` (required): Chat ID
- `message_id` (required): Message ID to edit
- `text` (required): New text
- `parse_mode` (optional): "Markdown" or "HTML"

**Example:**
```json
{
  "id": "edit_message_task",
  "action": "edit_message",
  "parameters": {
    "chat_id": "123456789",
    "message_id": 42,
    "text": "Updated message text"
  },
  "dependencies": ["send_message_task"]
}
```

#### 7. Delete Message

Delete message.

**Parameters:**
- `chat_id` (required): Chat ID
- `message_id` (required): Message ID to delete

**Example:**
```json
{
  "id": "delete_message_task",
  "action": "delete_message",
  "parameters": {
    "chat_id": "123456789",
    "message_id": 42
  },
  "dependencies": []
}
```

### Error Handling

- `AuthenticationError`: Invalid bot token
- `ChatNotFound`: Invalid chat_id
- `FileNotFound`: Cannot find file to send
- `MessageNotFound`: Cannot find message to edit/delete
- `RateLimitError`: Too many requests (respect Telegram limits)

---

## Skill Development Guide

### Creating a New Skill

To create a new skill, implement the `Skill` trait:

```rust
use async_trait::async_trait;
use jarvis_core::{Skill, TaskStep};

pub struct MyCustomSkill;

#[async_trait]
impl Skill for MyCustomSkill {
    fn name(&self) -> &str {
        "my_custom_skill"
    }

    fn description(&self) -> &str {
        "Description of what this skill does"
    }

    fn capabilities(&self) -> Vec<String> {
        vec![
            "action1".to_string(),
            "action2".to_string(),
        ]
    }

    fn can_handle(&self, task: &TaskStep) -> bool {
        self.capabilities().contains(&task.action)
    }

    async fn execute(&self, task: &TaskStep) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        match task.action.as_str() {
            "action1" => self.handle_action1(task).await,
            "action2" => self.handle_action2(task).await,
            _ => Err(format!("Unknown action: {}", task.action).into()),
        }
    }
}

impl MyCustomSkill {
    pub fn new() -> Self {
        Self
    }

    async fn handle_action1(&self, task: &TaskStep) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        // Implementation
        Ok(serde_json::json!({"status": "success"}))
    }

    async fn handle_action2(&self, task: &TaskStep) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        // Implementation
        Ok(serde_json::json!({"status": "success"}))
    }
}
```

### Best Practices

1. **Error Handling**: Always handle errors gracefully and return descriptive error messages
2. **Parameter Validation**: Validate all parameters before execution
3. **Async Operations**: Use async/await for I/O operations
4. **Logging**: Log important operations and errors
5. **Testing**: Write comprehensive tests for each action
6. **Documentation**: Document all actions, parameters, and return values
7. **Security**: Validate and sanitize all inputs, especially file paths and commands

### Testing Your Skill

```rust
#[tokio::test]
async fn test_my_skill() {
    let skill = MyCustomSkill::new();

    let task = TaskStep {
        id: "test".to_string(),
        action: "action1".to_string(),
        parameters: serde_json::json!({"param": "value"}),
        dependencies: vec![],
    };

    let result = skill.execute(&task).await;
    assert!(result.is_ok());
}
```

### Registering Your Skill

```rust
let mut orchestrator = JarvisOrchestrator::new();
orchestrator.register_skill(Arc::new(MyCustomSkill::new()));
```

---

## Performance Tips

1. **Parallel Execution**: Use `ExecutionMode::Parallel` for independent tasks
2. **Dependency Management**: Minimize dependencies between tasks
3. **Resource Pooling**: Reuse connections (SMTP, IMAP, Telegram)
4. **Caching**: Cache frequently used data
5. **Timeouts**: Set appropriate timeouts for long-running operations

## Security Considerations

1. **Credentials**: Never hardcode credentials - use environment variables
2. **Input Validation**: Always validate and sanitize inputs
3. **File Access**: Restrict file access to safe directories
4. **Rate Limiting**: Respect API rate limits (especially Telegram)
5. **Error Messages**: Don't expose sensitive information in error messages

## Troubleshooting

### Common Issues

**Email not sending:**
- Check SMTP credentials
- Verify firewall/antivirus settings
- Use app-specific passwords for Gmail
- Check SMTP port (587 for TLS, 465 for SSL)

**Telegram not working:**
- Verify bot token format
- Check internet connection
- Ensure chat_id is correct (can be negative for groups)
- Verify file paths for photos/documents

**Computer control not working:**
- Check platform-specific permissions (macOS accessibility)
- Verify display numbers
- Check for screen lock/screensaver

## Support and Contributing

For issues, feature requests, or contributions, please visit the project repository.

---

**Last Updated:** 2024-02-06
**Version:** 1.0.0
