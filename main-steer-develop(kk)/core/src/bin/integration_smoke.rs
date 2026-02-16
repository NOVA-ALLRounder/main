use std::time::Instant;

use local_os_agent::integrations::{gmail::GmailClient, notion::NotionClient, telegram::TelegramBot};
use tokio::time::{timeout, Duration};

fn short_err(e: &str) -> String {
    e.lines().next().unwrap_or(e).trim().to_string()
}

#[tokio::main]
async fn main() {
    let mut candidates = Vec::new();
    candidates.push(std::path::PathBuf::from("core/.env"));
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(parent) = cwd.parent() {
            candidates.push(parent.join(".env"));
            candidates.push(parent.join("core").join(".env"));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join(".env"));
            candidates.push(parent.join("core").join(".env"));
        }
    }

    println!("dotenv candidates:");
    for p in candidates {
        if p.exists() {
            match dotenv::from_path(&p) {
                Ok(_) => println!("  [OK] {}", p.display()),
                Err(e) => println!("  [ERR] {} => {}", p.display(), short_err(&e.to_string())),
            }
        } else {
            println!("  [MISS] {}", p.display());
        }
    }
    println!();

    local_os_agent::load_env();

    println!("=== Integration Smoke Test ===");
    println!("Targets: Gmail(list 3), Notion(query 1), Telegram(send 1)\n");
    if let Ok(cwd) = std::env::current_dir() {
        println!("CWD: {}", cwd.display());
    }
    let env_keys = [
        "NOTION_API_KEY",
        "NOTION_DATABASE_ID",
        "TELEGRAM_BOT_TOKEN",
        "TELEGRAM_CHAT_ID",
    ];
    for k in env_keys {
        match std::env::var(k) {
            Ok(v) if !v.trim().is_empty() => println!("ENV {}=SET(len={})", k, v.len()),
            Ok(_) => println!("ENV {}=SET_EMPTY", k),
            Err(_) => println!("ENV {}=MISSING", k),
        }
    }
    println!();

    let mut all_ok = true;

    // Gmail
    let started = Instant::now();
    let gmail_run = timeout(
        Duration::from_secs(90),
        tokio::spawn(async {
            let client = GmailClient::new()
                .await
                .map_err(|e| format!("client init failed: {}", short_err(&e.to_string())))?;
            let items = client
                .list_messages(3)
                .await
                .map_err(|e| format!("list_messages failed: {}", short_err(&e.to_string())))?;
            Ok::<Vec<(String, String, String)>, String>(items)
        }),
    )
    .await;
    match gmail_run {
        Ok(Ok(Ok(items))) => {
            println!(
                "[PASS] Gmail: fetched {} message(s) in {}ms",
                items.len(),
                started.elapsed().as_millis()
            );
            for (idx, (_id, subject, from)) in items.iter().take(3).enumerate() {
                println!(
                    "  {}. {} | {}",
                    idx + 1,
                    subject.chars().take(80).collect::<String>(),
                    from.chars().take(60).collect::<String>()
                );
            }
        }
        Ok(Ok(Err(err))) => {
            all_ok = false;
            println!(
                "[FAIL] Gmail: {} ({}ms)",
                err,
                started.elapsed().as_millis()
            );
        }
        Ok(Err(join_err)) => {
            all_ok = false;
            println!(
                "[FAIL] Gmail: panic/join error in {}ms: {}",
                started.elapsed().as_millis(),
                short_err(&join_err.to_string())
            );
        }
        Err(_) => {
            all_ok = false;
            println!(
                "[FAIL] Gmail: timeout after {}ms",
                started.elapsed().as_millis()
            );
        }
    }

    // Notion
    let started = Instant::now();
    let notion_db = std::env::var("NOTION_DATABASE_ID").unwrap_or_default();
    if notion_db.trim().is_empty() {
        all_ok = false;
        println!(
            "[FAIL] Notion: NOTION_DATABASE_ID missing ({}ms)",
            started.elapsed().as_millis()
        );
    } else {
        let notion_db_owned = notion_db.clone();
        let notion_run = timeout(
            Duration::from_secs(45),
            tokio::spawn(async move {
                let client = NotionClient::from_env()
                    .map_err(|e| format!("client init failed: {}", short_err(&e.to_string())))?;
                let pages = client
                    .query_database(&notion_db_owned, 1)
                    .await
                    .map_err(|e| format!("query failed: {}", short_err(&e.to_string())))?;
                Ok::<Vec<local_os_agent::integrations::notion::NotionPage>, String>(pages)
            }),
        )
        .await;
        match notion_run {
            Ok(Ok(Ok(pages))) => {
                println!(
                    "[PASS] Notion: query ok ({} page(s)) in {}ms",
                    pages.len(),
                    started.elapsed().as_millis()
                );
                if let Some(first) = pages.first() {
                    println!(
                        "  sample: {} ({})",
                        first.title.chars().take(80).collect::<String>(),
                        first.id
                    );
                }
            }
            Ok(Ok(Err(err))) => {
                all_ok = false;
                println!(
                    "[FAIL] Notion: {} ({}ms)",
                    err,
                    started.elapsed().as_millis()
                );
            }
            Ok(Err(join_err)) => {
                all_ok = false;
                println!(
                    "[FAIL] Notion: panic/join error in {}ms: {}",
                    started.elapsed().as_millis(),
                    short_err(&join_err.to_string())
                );
            }
            Err(_) => {
                all_ok = false;
                println!(
                    "[FAIL] Notion: timeout after {}ms",
                    started.elapsed().as_millis()
                );
            }
        }
    }

    // Telegram
    let started = Instant::now();
    let telegram_run = timeout(
        Duration::from_secs(30),
        tokio::spawn(async {
            let bot = TelegramBot::from_env()
                .map_err(|e| format!("client init failed: {}", short_err(&e.to_string())))?;
            let msg = format!(
                "Steer smoke test\nTime: {}\nChecks: gmail/notion/telegram",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            );
            bot.send(&msg)
                .await
                .map_err(|e| format!("send failed: {}", short_err(&e.to_string())))?;
            Ok::<(), String>(())
        }),
    )
    .await;
    match telegram_run {
        Ok(Ok(Ok(()))) => {
            println!(
                "[PASS] Telegram: sent test message in {}ms",
                started.elapsed().as_millis()
            );
        }
        Ok(Ok(Err(err))) => {
            all_ok = false;
            println!(
                "[FAIL] Telegram: {} ({}ms)",
                err,
                started.elapsed().as_millis(),
            );
        }
        Ok(Err(join_err)) => {
            all_ok = false;
            println!(
                "[FAIL] Telegram: panic/join error in {}ms: {}",
                started.elapsed().as_millis(),
                short_err(&join_err.to_string())
            );
        }
        Err(_) => {
            all_ok = false;
            println!(
                "[FAIL] Telegram: timeout after {}ms",
                started.elapsed().as_millis()
            );
        }
    }

    println!();
    if all_ok {
        println!("RESULT: PASS (all integrations reachable)");
        std::process::exit(0);
    } else {
        println!("RESULT: FAIL (see errors above)");
        std::process::exit(1);
    }
}
