use local_os_agent::{api_server, llm_gateway, policy};
use std::sync::Arc;

pub(crate) async fn handle_rewrite_fast_path(args: &[String]) -> anyhow::Result<bool> {
    if args.len() < 3 || args[1] != "rewrite" {
        return Ok(false);
    }

    let message = args[2..].join(" ");
    let refined = match llm_gateway::OpenAILLMClient::new() {
        Ok(c) => {
            let llm = Arc::new(c) as Arc<dyn llm_gateway::LLMClient>;
            if let Some(bot) = local_os_agent::telegram::TelegramBot::from_env(llm, None) {
                bot.improve_message(&message).await
            } else {
                message.clone()
            }
        }
        Err(_) => message.clone(),
    };
    println!("{}", refined);
    Ok(true)
}

pub(crate) async fn handle_post_init_fast_path(
    args: &[String],
    llm_client: Option<Arc<dyn llm_gateway::LLMClient>>,
) -> anyhow::Result<bool> {
    if args.len() >= 3 && args[1] == "notify" {
        let message = args[2..].join(" ");
        println!("🔔 Sending Smart Notification: {}", message);
        if let Some(llm) = llm_client {
            if let Some(bot) = local_os_agent::telegram::TelegramBot::from_env(llm, None) {
                let chat_id_str =
                    std::env::var("TELEGRAM_CHAT_ID").unwrap_or_else(|_| "0".to_string());
                if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                    if let Err(e) = bot.send_smart_notification(chat_id, &message).await {
                        eprintln!("❌ Failed to send notification: {}", e);
                    } else {
                        println!("✅ Notification sent successfully (Smart Mode).");
                    }
                } else {
                    eprintln!("❌ TELEGRAM_CHAT_ID not set or invalid.");
                }
            } else {
                eprintln!("❌ Telegram Bot configuration missing (TELEGRAM_BOT_TOKEN).");
            }
        } else {
            eprintln!("❌ LLM Client not available for smart notification.");
        }
        return Ok(true);
    }

    if args.len() >= 3 && args[1] == "surf" {
        let goal = args[2..].join(" ");
        println!("🎯 [CLI] Direct surf mode: {}", goal);
        if let Some(llm) = llm_client {
            let mut cli_policy = policy::PolicyEngine::new();
            cli_policy.unlock();
            let planner = local_os_agent::controller::planner::Planner::new(llm, None);
            match planner.run_goal_tracked(&goal, None).await {
                Ok(outcome) => println!(
                    "✅ Surf completed successfully! run_id={} planner={} execution={} business={}",
                    outcome.run_id,
                    outcome.planner_complete,
                    outcome.execution_complete,
                    outcome.business_complete
                ),
                Err(e) => println!("❌ Surf failed: {}", e),
            }
        } else {
            println!("❌ LLM not available for surf mode");
        }
        return Ok(true);
    }

    if args.len() >= 2 && (args[1] == "telegram_listen" || args[1] == "telegram-listen") {
        if let Some(llm) = llm_client {
            if let Some(bot) = local_os_agent::telegram::TelegramBot::from_env(llm, None) {
                println!("🤖 Telegram listener mode started. Waiting for commands...");
                Arc::new(bot).start_polling().await;
            } else {
                eprintln!(
                    "❌ Telegram listener requires TELEGRAM_BOT_TOKEN (optional: TELEGRAM_USER_ID)."
                );
            }
        } else {
            eprintln!("❌ LLM not available for Telegram listener.");
        }
        return Ok(true);
    }

    if args.len() >= 2 && args[1] == "telegram_api_listen" {
        if let Some(llm) = llm_client {
            match api_server::try_spawn_telegram_listener(llm) {
                Ok(api_server::TelegramListenerStartOutcome::Started) => {
                    println!("🤖 Telegram listener started.");
                }
                Ok(api_server::TelegramListenerStartOutcome::AlreadyRunning) => {
                    println!("ℹ️  Telegram listener is already running.");
                }
                Err("missing_telegram_token") => {
                    eprintln!(
                        "❌ Telegram listener requires TELEGRAM_BOT_TOKEN (optional: TELEGRAM_USER_ID)."
                    );
                }
                Err(_) => {
                    eprintln!("❌ Telegram listener start failed (unknown error).");
                }
            }
        } else {
            eprintln!("❌ LLM not available for Telegram listener.");
        }
        return Ok(true);
    }

    Ok(false)
}
