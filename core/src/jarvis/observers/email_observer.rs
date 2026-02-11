// Email Observer - Monitor inbox for new emails via IMAP

use super::{Observation, Observer, Priority};
use anyhow::Result;
use serde_json::json;
use std::time::SystemTime;

pub struct EmailObserver {
    imap_server: String,
    username: String,
    password: String,
    last_check: SystemTime,
    check_interval_secs: u64,
}

impl EmailObserver {
    pub fn new() -> Result<Self> {
        dotenvy::dotenv().ok();

        let imap_server = std::env::var("EMAIL_IMAP_SERVER")
            .unwrap_or_else(|_| "imap.gmail.com:993".to_string());
        let username = std::env::var("EMAIL_USERNAME")
            .unwrap_or_default();
        let password = std::env::var("EMAIL_PASSWORD")
            .unwrap_or_default();

        if username.is_empty() || password.is_empty() {
            log::warn!("Email credentials not set. EmailObserver will not function.");
            log::warn!("Set EMAIL_USERNAME and EMAIL_PASSWORD in .env");
        }

        Ok(Self {
            imap_server,
            username,
            password,
            last_check: SystemTime::now(),
            check_interval_secs: 60, // Check every 60 seconds
        })
    }

    fn should_check(&self) -> bool {
        match self.last_check.elapsed() {
            Ok(duration) => duration.as_secs() >= self.check_interval_secs,
            Err(_) => true,
        }
    }

    async fn fetch_new_emails(&mut self) -> Result<Vec<EmailData>> {
        if self.username.is_empty() || self.password.is_empty() {
            return Ok(Vec::new());
        }

        // TODO: Implement async IMAP with async-imap crate
        // For now, return empty to allow build to succeed
        // Phase 3 will add full email integration

        self.last_check = SystemTime::now();
        Ok(Vec::new())
    }

    fn detect_priority(&self, email: &EmailData) -> Priority {
        let subject_lower = email.subject.to_lowercase();
        let from_lower = email.from.to_lowercase();

        // Urgent keywords
        if subject_lower.contains("urgent")
            || subject_lower.contains("asap")
            || subject_lower.contains("疫뀀떯??)
            || subject_lower.contains("筌앸맩??)
        {
            return Priority::Urgent;
        }

        // Important senders (customize this)
        let important_domains = vec!["boss", "ceo", "director", "manager"];
        if important_domains.iter().any(|d| from_lower.contains(d)) {
            return Priority::High;
        }

        // Has attachment
        if email.has_attachment {
            return Priority::Medium;
        }

        // Reply keywords
        if subject_lower.starts_with("re:") || subject_lower.starts_with("???삢:") {
            return Priority::Medium;
        }

        Priority::Low
    }
}

#[derive(Debug, Clone)]
struct EmailData {
    from: String,
    subject: String,
    preview: String,
    has_attachment: bool,
    received_at: i64,
}

#[async_trait::async_trait]
impl Observer for EmailObserver {
    async fn observe(&self) -> Result<Vec<Observation>> {
        if !self.should_check() {
            return Ok(Vec::new());
        }

        let mut this = self.clone();
        let emails = this.fetch_new_emails().await?;

        let observations: Vec<Observation> = emails
            .iter()
            .map(|email| {
                let priority = self.detect_priority(email);

                Observation {
                    source: "email".to_string(),
                    event_type: "new_email".to_string(),
                    priority,
                    data: json!({
                        "from": email.from,
                        "subject": email.subject,
                        "preview": email.preview,
                        "has_attachment": email.has_attachment,
                    }),
                    timestamp: email.received_at,
                }
            })
            .collect();

        if !observations.is_empty() {
            log::info!("?踰?Detected {} new emails", observations.len());
        }

        Ok(observations)
    }

    fn name(&self) -> &str {
        "EmailObserver"
    }
}

// Need Clone for spawn_blocking
impl Clone for EmailObserver {
    fn clone(&self) -> Self {
        Self {
            imap_server: self.imap_server.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            last_check: self.last_check,
            check_interval_secs: self.check_interval_secs,
        }
    }
}
