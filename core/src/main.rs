use local_os_agent::singleton_lock;
mod main_bootstrap;
mod main_commands;
mod main_fast_paths;
mod main_repl;
mod main_support;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize Tracing
    tracing_subscriber::fmt::init();
    local_os_agent::load_env_with_fallback();

    main_bootstrap::install_panic_hook();

    // Fast-path: keep `rewrite` output clean (no startup banners/log noise).
    let args: Vec<String> = std::env::args().collect();
    if main_fast_paths::handle_rewrite_fast_path(&args).await? {
        return Ok(());
    }

    let _lock = match singleton_lock::acquire_lock() {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!("⛔️ {}", err);
            return Ok(());
        }
    };

    main_bootstrap::print_startup_banner_and_checks().await;
    let llm_client = main_bootstrap::init_db_and_llm();

    // Fast-path CLI commands: run before background services (API/EventTap/Watchers)
    // so they are not blocked by API port conflicts.
    if main_fast_paths::handle_post_init_fast_path(&args, llm_client.clone()).await? {
        return Ok(());
    }

    let log_tx = main_bootstrap::start_background_services(llm_client.clone()).await;
    main_repl::run_repl_loop(llm_client, log_tx).await
}
