// EmailSkill - Email management (Gmail/IMAP)
// Send, list, search emails

use super::{Skill, SkillContext, SkillResult};
use crate::jarvis::skills::metadata::*;
use async_trait::async_trait;
use serde_json::json;
use lettre::{
    Message, SmtpTransport, Transport,
    transport::smtp::authentication::Credentials,
};
use tokio_util::compat::TokioAsyncReadCompatExt;
use futures::StreamExt;

pub struct EmailSkill {
    smtp_username: String,
    smtp_password: String,
    smtp_server: String,
    smtp_port: u16,
}

impl EmailSkill {
    pub fn new() -> Self {
        let smtp_username = std::env::var("GMAIL_USER").unwrap_or_default();
        let smtp_password = std::env::var("GMAIL_APP_PASSWORD").unwrap_or_default();
        let smtp_server = std::env::var("SMTP_SERVER").unwrap_or_else(|_| "smtp.gmail.com".to_string());
        let smtp_port = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(587);

        if smtp_username.is_empty() || smtp_password.is_empty() {
            log::warn!("Email credentials not set - EmailSkill may not work properly");
        }

        Self {
            smtp_username,
            smtp_password,
            smtp_server,
            smtp_port,
        }
    }

    fn validate_credentials(&self) -> Result<(), String> {
        if self.smtp_username.is_empty() {
            return Err("GMAIL_USER not set".to_string());
        }
        if self.smtp_password.is_empty() {
            return Err("GMAIL_APP_PASSWORD not set".to_string());
        }
        Ok(())
    }
}

impl Default for EmailSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for EmailSkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "email".to_string(),
            description: "Manage emails: send, list, search".to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "send".to_string(),
                "list".to_string(),
                "search".to_string(),
                "read".to_string(),
            ],
            requirements: SkillRequirements {
                env_vars: vec![
                    "GMAIL_USER".to_string(),
                    "GMAIL_APP_PASSWORD".to_string(),
                ],
                required_bins: vec![],
                any_bins: vec![],
                platform: None, // Cross-platform
                config_keys: vec![],
            },
            tags: vec!["communication".to_string(), "productivity".to_string()],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        let requirements = self.metadata().requirements;
        check_requirements(&requirements)
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        log::info!(
            "EmailSkill executing action: {} (session: {})",
            ctx.action,
            ctx.session_key
        );

        match ctx.action.as_str() {
            "send" => self.send_email(ctx).await,
            "list" => self.list_emails(ctx).await,
            "search" => self.search_emails(ctx).await,
            "read" => self.read_email(ctx).await,
            _ => SkillResult::error(format!("Unknown action: {}", ctx.action)),
        }
    }

    fn requires_approval(&self, action: &str) -> bool {
        // Sending emails requires approval
        action == "send"
    }
}

impl EmailSkill {
    async fn send_email(&self, ctx: SkillContext) -> SkillResult {
        // Validate credentials
        if let Err(e) = self.validate_credentials() {
            return SkillResult::error(format!("Credential validation failed: {}", e));
        }

        let to = match ctx.params.get("to") {
            Some(serde_json::Value::String(s)) => s.clone(),
            _ => {
                return SkillResult::error("Missing 'to' parameter");
            }
        };

        let subject = ctx
            .params
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("(no subject)")
            .to_string();

        let body = ctx
            .params
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        log::info!("Sending email to {} with subject '{}'", to, subject);

        // Build email message
        let from_addr = match self.smtp_username.parse() {
            Ok(addr) => addr,
            Err(e) => {
                log::error!("Invalid from address: {}", e);
                return SkillResult::error(format!("Invalid from address: {}", e));
            }
        };

        let to_addr = match to.parse() {
            Ok(addr) => addr,
            Err(e) => {
                log::error!("Invalid to address: {}", e);
                return SkillResult::error(format!("Invalid to address: {}", e));
            }
        };

        let email = match Message::builder()
            .from(from_addr)
            .to(to_addr)
            .subject(&subject)
            .body(body.clone())
        {
            Ok(email) => email,
            Err(e) => {
                log::error!("Failed to build email: {}", e);
                return SkillResult::error(format!("Failed to build email: {}", e));
            }
        };

        // Create SMTP credentials
        let creds = Credentials::new(
            self.smtp_username.clone(),
            self.smtp_password.clone(),
        );

        // Create SMTP transport with STARTTLS
        let mailer = match SmtpTransport::starttls_relay(&self.smtp_server) {
            Ok(transport) => transport
                .credentials(creds)
                .port(self.smtp_port)
                .build(),
            Err(e) => {
                log::error!("Failed to create SMTP transport: {}", e);
                return SkillResult::error(format!("Failed to create SMTP transport: {}", e));
            }
        };

        // Send the email
        match mailer.send(&email) {
            Ok(_) => {
                log::info!("Email sent successfully to {}", to);
                let timestamp = chrono::Utc::now().to_rfc3339();
                SkillResult::success_with_data(
                    format!("Email sent to {}", to),
                    json!({
                        "to": to,
                        "subject": subject,
                        "status": "sent",
                        "timestamp": timestamp
                    }),
                )
            }
            Err(e) => {
                log::error!("Failed to send email: {}", e);
                SkillResult::error(format!("Failed to send email: {}", e))
            }
        }
    }

    async fn list_emails(&self, ctx: SkillContext) -> SkillResult {
        // Validate credentials
        if let Err(e) = self.validate_credentials() {
            return SkillResult::error(format!("Credential validation failed: {}", e));
        }

        let limit = ctx
            .params
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(10) as usize;

        log::info!("Fetching {} recent emails via IMAP", limit);

        // Connect to IMAP server with TLS
        let domain = "imap.gmail.com";
        let port = 993;

        // Connect to TCP socket
        let tcp_stream = match tokio::net::TcpStream::connect((domain, port)).await {
            Ok(stream) => stream,
            Err(e) => {
                log::error!("Failed to connect to {}:{}: {}", domain, port, e);
                return SkillResult::error(format!("Failed to connect to IMAP server: {}", e));
            }
        };

        // Convert tokio stream to futures-compatible stream
        let compat_tcp = tcp_stream.compat();

        // Wrap with TLS
        let tls = async_native_tls::TlsConnector::new();
        let tls_stream = match tls.connect(domain, compat_tcp).await {
            Ok(stream) => stream,
            Err(e) => {
                log::error!("Failed to establish TLS connection: {}", e);
                return SkillResult::error(format!("Failed to establish TLS: {}", e));
            }
        };

        // Create IMAP client
        let client = async_imap::Client::new(tls_stream);

        // Login
        let mut session = match client
            .login(&self.smtp_username, &self.smtp_password)
            .await
        {
            Ok(session) => session,
            Err((e, _)) => {
                log::error!("Failed to login to IMAP: {}", e);
                return SkillResult::error(format!("Failed to login to IMAP: {}", e));
            }
        };

        // Select INBOX
        if let Err(e) = session.select("INBOX").await {
            log::error!("Failed to select INBOX: {}", e);
            let _ = session.logout().await;
            return SkillResult::error(format!("Failed to select INBOX: {}", e));
        }

        // Fetch and process emails in a separate scope
        let emails = {
            // Fetch recent emails
            let sequence = format!("1:*");
            let mut fetch_stream = match session.fetch(sequence, "ENVELOPE").await {
                Ok(stream) => stream,
                Err(e) => {
                    log::error!("Failed to fetch emails: {}", e);
                    // Can't logout here due to borrow rules
                    return SkillResult::error(format!("Failed to fetch emails: {}", e));
                }
            };

            // Collect all messages first
            let mut all_messages = Vec::new();
            while let Some(result) = fetch_stream.next().await {
                match result {
                    Ok(msg) => all_messages.push(msg),
                    Err(e) => {
                        log::warn!("Error fetching message: {}", e);
                    }
                }
            }

            // Drop the stream explicitly to release the borrow
            drop(fetch_stream);

            let mut emails = Vec::new();
            let mut count = 0;

            // Process messages in reverse order (most recent first)
            for msg in all_messages.iter().rev() {
                if count >= limit {
                    break;
                }

                if let Some(envelope) = msg.envelope() {
                    let subject = envelope
                        .subject
                        .as_ref()
                        .and_then(|s| std::str::from_utf8(s).ok())
                        .unwrap_or("(no subject)")
                        .to_string();

                    let from = envelope
                        .from
                        .as_ref()
                        .and_then(|addrs| addrs.first())
                        .and_then(|addr| {
                            addr.mailbox
                                .as_ref()
                                .and_then(|m| std::str::from_utf8(m).ok())
                                .map(|mailbox| {
                                    let host = addr
                                        .host
                                        .as_ref()
                                        .and_then(|h| std::str::from_utf8(h).ok())
                                        .unwrap_or("");
                                    format!("{}@{}", mailbox, host)
                                })
                        })
                        .unwrap_or("unknown".to_string());

                    emails.push(json!({
                        "id": msg.message,
                        "subject": subject,
                        "from": from,
                    }));

                    count += 1;
                }
            }

            emails
        }; // fetch_stream is dropped here, releasing session borrow

        // Now we can logout
        if let Err(e) = session.logout().await {
            log::warn!("Failed to logout from IMAP: {}", e);
        }

        log::info!("Retrieved {} emails", emails.len());

        SkillResult::success_with_data(
            format!("Listed {} emails", emails.len()),
            json!({
                "count": emails.len(),
                "emails": emails
            }),
        )
    }

    async fn search_emails(&self, ctx: SkillContext) -> SkillResult {
        let query = match ctx.params.get("query") {
            Some(serde_json::Value::String(s)) => s.clone(),
            _ => {
                return SkillResult::error("Missing 'query' parameter");
            }
        };

        // TODO: Implement email search
        log::info!("Would search emails for: {}", query);

        SkillResult::success_with_data(
            format!("Searched for '{}'", query),
            json!({
                "query": query,
                "results": []
            }),
        )
    }

    async fn read_email(&self, ctx: SkillContext) -> SkillResult {
        let email_id = match ctx.params.get("id") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Number(n)) => n.to_string(),
            _ => {
                return SkillResult::error("Missing 'id' parameter");
            }
        };

        // TODO: Fetch email content
        log::info!("Would read email: {}", email_id);

        SkillResult::success_with_data(
            format!("Read email {}", email_id),
            json!({
                "id": email_id,
                "subject": "Example",
                "from": "example@example.com",
                "body": "Email content..."
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::jarvis::models::context::UserContext;
    use std::time::Instant;

    fn test_user_context() -> UserContext {
        UserContext {
            active_app: None,
            active_window_title: None,
            last_activity: Instant::now(),
            activity_count: 0,
        }
    }

    #[tokio::test]
    async fn test_metadata() {
        let skill = EmailSkill::new();
        let meta = skill.metadata();

        assert_eq!(meta.name, "email");
        assert!(meta.actions.contains(&"send".to_string()));
        assert!(meta.requirements.env_vars.contains(&"GMAIL_USER".to_string()));
        assert!(meta.requirements.env_vars.contains(&"GMAIL_APP_PASSWORD".to_string()));
    }

    #[test]
    fn test_requires_approval() {
        let skill = EmailSkill::new();

        assert!(skill.requires_approval("send"));
        assert!(!skill.requires_approval("list"));
        assert!(!skill.requires_approval("search"));
    }

    #[test]
    fn test_credential_validation() {
        // Test with empty credentials
        let skill = EmailSkill {
            smtp_username: String::new(),
            smtp_password: String::new(),
            smtp_server: "smtp.gmail.com".to_string(),
            smtp_port: 587,
        };
        assert!(skill.validate_credentials().is_err());

        // Test with only username
        let skill = EmailSkill {
            smtp_username: "test@gmail.com".to_string(),
            smtp_password: String::new(),
            smtp_server: "smtp.gmail.com".to_string(),
            smtp_port: 587,
        };
        assert!(skill.validate_credentials().is_err());

        // Test with valid credentials
        let skill = EmailSkill {
            smtp_username: "test@gmail.com".to_string(),
            smtp_password: "password".to_string(),
            smtp_server: "smtp.gmail.com".to_string(),
            smtp_port: 587,
        };
        assert!(skill.validate_credentials().is_ok());
    }

    #[tokio::test]
    async fn test_send_email_missing_credentials() {
        let skill = EmailSkill {
            smtp_username: String::new(),
            smtp_password: String::new(),
            smtp_server: "smtp.gmail.com".to_string(),
            smtp_port: 587,
        };

        let mut params = HashMap::new();
        params.insert("to".to_string(), json!("test@example.com"));
        params.insert("subject".to_string(), json!("Test"));
        params.insert("body".to_string(), json!("Test body"));

        let ctx = SkillContext {
            action: "send".to_string(),
            params,
            session_key: "test".to_string(),
            user_context: test_user_context(),
        };

        let result = skill.send_email(ctx).await;
        assert!(!result.success);
        assert!(result.message.contains("Credential validation failed"));
    }

    #[tokio::test]
    async fn test_send_email_missing_to_parameter() {
        let skill = EmailSkill {
            smtp_username: "test@gmail.com".to_string(),
            smtp_password: "password".to_string(),
            smtp_server: "smtp.gmail.com".to_string(),
            smtp_port: 587,
        };

        let params = HashMap::new();

        let ctx = SkillContext {
            action: "send".to_string(),
            params,
            session_key: "test".to_string(),
            user_context: test_user_context(),
        };

        let result = skill.send_email(ctx).await;
        assert!(!result.success);
        assert!(result.message.contains("Missing 'to' parameter"));
    }
}
