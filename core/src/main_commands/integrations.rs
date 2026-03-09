use local_os_agent::{ai_digest, api_server, applescript, integrations, llm_gateway};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::main_support::run_gmail_digest_pipeline;

pub(crate) fn handle_control_command(parts: &[&str]) {
    if parts.len() < 3 {
        println!("Usage: control <app> <action> (e.g., control Music play)");
        return;
    }
    let app = parts[1];
    let command = parts[2];
    println!("🎮 Controlling {} with '{}'...", app, command);
    match applescript::control_app(app, command) {
        Ok(out) => {
            if !out.is_empty() {
                println!("Output: {}", out);
            }
            println!("✅ Command sent.");
        }
        Err(e) => println!("❌ Control failed: {}", e),
    }
}

pub(crate) async fn handle_ai_digest_command(parts: &[&str]) {
    let request = if parts.len() >= 2 {
        parts[1..].join(" ")
    } else {
        ai_digest::default_request_text().to_string()
    };
    println!("📰 Triggering news digest workflow...");
    match ai_digest::trigger_program_webhook(&request, None).await {
        Ok(result) => println!("{}", ai_digest::format_human_summary(&result)),
        Err(e) => println!("❌ News digest trigger failed: {}", e),
    }
}

pub(crate) async fn handle_telegram_command(parts: &[&str]) {
    if parts.len() < 2 {
        println!("Usage: telegram <message>");
        return;
    }
    let message = parts[1..].join(" ");
    println!("📱 Sending to Telegram...");
    match integrations::telegram::TelegramBot::from_env() {
        Ok(bot) => match bot.send(&message).await {
            Ok(_) => println!("✅ Message sent!"),
            Err(e) => println!("❌ Failed: {}", e),
        },
        Err(e) => println!("⚠️  Telegram not configured: {}", e),
    }
}

pub(crate) fn handle_telegram_listener_command(
    llm_client: Option<Arc<dyn llm_gateway::LLMClient>>,
) {
    if let Some(llm) = llm_client {
        match api_server::try_spawn_telegram_listener(llm) {
            Ok(api_server::TelegramListenerStartOutcome::Started) => {
                println!("🤖 Telegram listener started.");
            }
            Ok(api_server::TelegramListenerStartOutcome::AlreadyRunning) => {
                println!("ℹ️  Telegram listener is already running.");
            }
            Err("missing_telegram_token") => {
                println!(
                    "⚠️  Telegram listener requires TELEGRAM_BOT_TOKEN (optional: TELEGRAM_USER_ID)."
                );
            }
            Err(_) => {
                println!("❌ Telegram listener start failed (unknown error).");
            }
        }
    } else {
        println!("⚠️  LLM Client not available.");
    }
}

pub(crate) async fn handle_notion_command(parts: &[&str]) {
    if parts.len() < 2 {
        println!("Usage: notion <title> | <content>");
        return;
    }
    let full_text = parts[1..].join(" ");
    let split: Vec<&str> = full_text.splitn(2, '|').collect();
    let title = split.first().unwrap_or(&"Untitled").trim();
    let content = split.get(1).unwrap_or(&"").trim();

    let db_id = std::env::var("NOTION_DATABASE_ID").unwrap_or_default();
    if db_id.is_empty() {
        println!("⚠️  NOTION_DATABASE_ID not set in .env");
        return;
    }

    println!("📝 Creating Notion page: '{}'...", title);
    match integrations::notion::NotionClient::from_env() {
        Ok(client) => match client.create_page(&db_id, title, content).await {
            Ok(page_id) => println!("✅ Page created! ID: {}", page_id),
            Err(e) => println!("❌ Failed: {}", e),
        },
        Err(e) => println!("⚠️  Notion not configured: {}", e),
    }
}

pub(crate) async fn handle_gmail_command(
    parts: &[&str],
    llm_client: Option<&dyn llm_gateway::LLMClient>,
) {
    if parts.len() < 2 {
        println!(
            "Usage: gmail list [N] | gmail read <id> | gmail send <to>|<subj>|<body> | gmail digest [N]"
        );
        return;
    }
    match parts[1] {
        "list" => {
            let count = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
            println!("📧 Fetching {} recent emails...", count);
            match integrations::gmail::GmailClient::new().await {
                Ok(client) => match client.list_messages(count).await {
                    Ok(messages) => {
                        if messages.is_empty() {
                            println!("   (No messages found)");
                        } else {
                            for (id, subject, from) in messages {
                                println!(
                                    "  📩 [{}] {} — {}",
                                    &id[..8.min(id.len())],
                                    subject,
                                    from
                                );
                            }
                        }
                    }
                    Err(e) => println!("❌ Failed: {}", e),
                },
                Err(e) => println!("⚠️  Gmail auth failed: {}", e),
            }
        }
        "read" => {
            if parts.len() < 3 {
                println!("Usage: gmail read <id>");
                return;
            }
            let id = parts[2];
            println!("📖 Reading email {}...", id);
            match integrations::gmail::GmailClient::new().await {
                Ok(client) => match client.get_message(id).await {
                    Ok(content) => println!("\n{}", content),
                    Err(e) => println!("❌ Failed: {}", e),
                },
                Err(e) => println!("⚠️  Gmail auth failed: {}", e),
            }
        }
        "send" => {
            let full_text = parts[2..].join(" ");
            let split: Vec<&str> = full_text.splitn(3, '|').collect();
            if split.len() < 3 {
                println!("Usage: gmail send <to>|<subject>|<body>");
                return;
            }
            let to = split[0].trim();
            let subject = split[1].trim();
            let body = split[2].trim();

            println!("✉️  Sending email to {}...", to);
            match integrations::gmail::GmailClient::new().await {
                Ok(client) => match client.send_message(to, subject, body).await {
                    Ok(id) => println!("✅ Email sent! ID: {}", id),
                    Err(e) => println!("❌ Failed: {}", e),
                },
                Err(e) => println!("⚠️  Gmail auth failed: {}", e),
            }
        }
        "digest" | "digest5" => {
            let count = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
            println!("🧠 Running gmail digest pipeline ({} mails)...", count);
            match run_gmail_digest_pipeline(count, llm_client).await {
                Ok(()) => {}
                Err(e) => println!("❌ Digest failed: {}", e),
            }
        }
        _ => println!("Unknown gmail subcommand. Use: list, read, send, digest"),
    }
}

pub(crate) async fn handle_calendar_command(parts: &[&str]) {
    if parts.len() < 2 {
        println!("Usage: calendar today | week | add <title>|<start>|<end>");
        return;
    }
    match parts[1] {
        "today" => {
            println!("📅 Fetching today's events...");
            match integrations::calendar::CalendarClient::new().await {
                Ok(client) => match client.list_today().await {
                    Ok(events) => {
                        if events.is_empty() {
                            println!("   (No events today)");
                        } else {
                            for (_, summary, time) in events {
                                println!("  🗓️  {} — {}", time, summary);
                            }
                        }
                    }
                    Err(e) => println!("❌ Failed: {}", e),
                },
                Err(e) => println!("⚠️  Calendar auth failed: {}", e),
            }
        }
        "week" => {
            println!("📅 Fetching this week's events...");
            match integrations::calendar::CalendarClient::new().await {
                Ok(client) => match client.list_week().await {
                    Ok(events) => {
                        if events.is_empty() {
                            println!("   (No events this week)");
                        } else {
                            for (_, summary, time) in events {
                                println!("  🗓️  {} — {}", time, summary);
                            }
                        }
                    }
                    Err(e) => println!("❌ Failed: {}", e),
                },
                Err(e) => println!("⚠️  Calendar auth failed: {}", e),
            }
        }
        "add" => {
            let full_text = parts[2..].join(" ");
            let split: Vec<&str> = full_text.splitn(3, '|').collect();
            if split.len() < 3 {
                println!("Usage: calendar add <title>|<start ISO>|<end ISO>");
                println!("Example: calendar add Meeting|2026-01-25T14:00:00+09:00|2026-01-25T15:00:00+09:00");
                return;
            }
            let title = split[0].trim();
            let start = split[1].trim();
            let end = split[2].trim();

            info!("➕ Adding event: '{}'...", title);
            match integrations::calendar::CalendarClient::new().await {
                Ok(client) => match client.create_event(title, start, end).await {
                    Ok(id) => info!("✅ Event created! ID: {}", id),
                    Err(e) => error!("❌ Failed: {}", e),
                },
                Err(e) => warn!("⚠️  Calendar auth failed: {}", e),
            }
        }
        _ => warn!("Unknown calendar subcommand. Use: today, week, add"),
    }
}
