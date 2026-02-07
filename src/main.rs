//! CogniArk - A secure, self-improving AI agent
//!
//! Features:
//! - Parallel thinking with multiple reasoning paths
//! - Sub-agent orchestration (Researcher, Coder, Analyst, etc.)
//! - Cognitive memory (episodic/semantic/procedural)
//! - Cryptographic execution proofs
//! - Sandboxed action execution (WASM + Docker)
//! - Native GUI (egui) + Telegram integration
//! - Local-first HTTP API

mod core;
mod crypto;
mod security;
mod storage;
mod memory;
mod identity;
mod safety;
mod proofs;
mod runtime;
mod channels;
mod actions;
mod integrations;

#[cfg(feature = "gui")]
mod gui;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::io::{Write, BufRead};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "cogniark")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run in headless mode (no GUI)
    #[arg(long)]
    headless: bool,

    /// Configuration directory
    #[arg(long, env = "COGNIARK_CONFIG")]
    config: Option<PathBuf>,

    /// Data directory
    #[arg(long, env = "COGNIARK_DATA")]
    data: Option<PathBuf>,

    /// Run the setup wizard
    #[arg(long)]
    setup: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing with secure defaults
    // Suppress SQLx query logs to prevent sensitive data exposure
    let default_filter = format!(
        "{},sqlx::query=warn,sea_orm=warn,hyper=warn,reqwest=warn",
        args.log_level
    );
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| default_filter.parse().expect("Invalid log filter")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Determine directories
    let dirs = directories::ProjectDirs::from("com", "cogniark", "CogniArk")
        .expect("Failed to determine project directories");

    let config_dir = args.config.unwrap_or_else(|| dirs.config_dir().to_path_buf());
    let data_dir = args.data.unwrap_or_else(|| dirs.data_dir().to_path_buf());

    // Ensure directories exist
    std::fs::create_dir_all(&config_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    // Check if this is first run (no config file exists)
    let config_path = config_dir.join("config.toml");
    let is_first_run = !config_path.exists();

    if is_first_run && !args.setup {
        // Print welcome message
        println!();
        println!("╔═══════════════════════════════════════════════════════════╗");
        println!("║                                                           ║");
        println!("║                Welcome to CogniArk v{}                 ║", env!("CARGO_PKG_VERSION"));
        println!("║                                                           ║");
        println!("║   A secure, self-improving AI agent with:                 ║");
        println!("║   • Parallel thinking (multiple reasoning paths)          ║");
        println!("║   • Sub-agent orchestration                               ║");
        println!("║   • Cognitive memory (episodic/semantic/procedural)       ║");
        println!("║   • Sandboxed action execution (WASM/Docker)              ║");
        println!("║                                                           ║");
        println!("╚═══════════════════════════════════════════════════════════╝");
        println!();
    }

    tracing::info!("Starting CogniArk v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("Config directory: {}", config_dir.display());
    tracing::info!("Data directory: {}", data_dir.display());

    // Initialize core systems
    let agent = core::Agent::init(&config_dir, &data_dir).await?;
    tracing::info!("Agent DID: {}", agent.identity.did());

    // Handle first run or explicit setup
    if args.setup || is_first_run {
        // In headless mode (Docker), skip interactive setup - just use defaults
        // Users can configure via the Web UI Settings page
        if args.headless && !args.setup {
            tracing::info!("First run in headless mode - using default config");
            tracing::info!("Configure via Web UI at http://127.0.0.1:17990 -> Settings");
            // Config already has defaults, just save it
            agent.config.save(&config_dir)?;
        } else {
            #[cfg(feature = "gui")]
            if !args.headless {
                println!("Launching setup wizard...");
                gui::run_setup_wizard(agent).await?;
                return Ok(());
            }

            // CLI setup for explicit --setup flag
            run_cli_setup(&config_dir, &agent).await?;

            // Reload the agent with new config and continue
            let agent = core::Agent::init(&config_dir, &data_dir).await?;
            return run_headless(agent).await;
        }
    }

    if args.headless {
        run_headless(agent).await
    } else {
        #[cfg(feature = "gui")]
        {
            gui::run(agent).await
        }
        #[cfg(not(feature = "gui"))]
        {
            tracing::warn!("GUI feature not enabled, running headless");
            run_headless(agent).await
        }
    }
}

/// CLI-based setup wizard for headless mode
async fn run_cli_setup(config_dir: &PathBuf, agent: &core::Agent) -> Result<()> {
    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("                    SETUP WIZARD");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("Your Agent Identity (DID):");
    println!("  {}", agent.identity.did());
    println!();

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    // Step 1: LLM Provider
    println!("═══ Step 1: LLM Configuration ═══");
    println!();
    println!("Choose your LLM provider:");
    println!("  1) Ollama (local, free, private)");
    println!("  2) Anthropic Claude (cloud, most capable)");
    println!("  3) OpenAI GPT (cloud)");
    println!("  4) OpenAI-Compatible (LMStudio, vLLM, etc.)");
    println!();

    print!("Enter choice [1-4] (default: 1): ");
    stdout.flush()?;

    let mut choice = String::new();
    stdin.lock().read_line(&mut choice)?;
    let choice = choice.trim();

    let llm = match choice {
        "2" => {
            print!("Anthropic API Key: ");
            stdout.flush()?;
            let mut api_key = String::new();
            stdin.lock().read_line(&mut api_key)?;
            core::LlmProvider::Anthropic {
                api_key: api_key.trim().to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
            }
        }
        "3" => {
            print!("OpenAI API Key: ");
            stdout.flush()?;
            let mut api_key = String::new();
            stdin.lock().read_line(&mut api_key)?;
            core::LlmProvider::OpenAI {
                api_key: api_key.trim().to_string(),
                model: "gpt-4o".to_string(),
                base_url: None,
            }
        }
        "4" => {
            print!("Base URL (e.g., http://localhost:1234/v1): ");
            stdout.flush()?;
            let mut base_url = String::new();
            stdin.lock().read_line(&mut base_url)?;

            print!("Model name: ");
            stdout.flush()?;
            let mut model = String::new();
            stdin.lock().read_line(&mut model)?;

            core::LlmProvider::OpenAI {
                api_key: "not-needed".to_string(),
                model: model.trim().to_string(),
                base_url: Some(base_url.trim().to_string()),
            }
        }
        _ => {
            print!("Ollama URL (default: http://localhost:11434): ");
            stdout.flush()?;
            let mut url = String::new();
            stdin.lock().read_line(&mut url)?;
            let url = url.trim();
            let url = if url.is_empty() { "http://localhost:11434" } else { url };

            print!("Model (default: llama3.2): ");
            stdout.flush()?;
            let mut model = String::new();
            stdin.lock().read_line(&mut model)?;
            let model = model.trim();
            let model = if model.is_empty() { "llama3.2" } else { model };

            core::LlmProvider::Ollama {
                base_url: url.to_string(),
                model: model.to_string(),
            }
        }
    };

    println!();

    // Step 2: Telegram (optional)
    println!("═══ Step 2: Telegram Configuration (Optional) ═══");
    println!();
    print!("Configure Telegram bot? [y/N]: ");
    stdout.flush()?;

    let mut telegram_choice = String::new();
    stdin.lock().read_line(&mut telegram_choice)?;

    let telegram = if telegram_choice.trim().to_lowercase() == "y" {
        print!("Bot Token (from @BotFather): ");
        stdout.flush()?;
        let mut token = String::new();
        stdin.lock().read_line(&mut token)?;

        print!("Allowed User IDs (comma-separated, or empty for pairing mode): ");
        stdout.flush()?;
        let mut users = String::new();
        stdin.lock().read_line(&mut users)?;

        let allowed_users: Vec<i64> = users
            .trim()
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        Some(core::config::TelegramConfig {
            bot_token: token.trim().to_string(),
            allowed_users,
            dm_policy: "pairing".to_string(),
        })
    } else {
        None
    };

    println!();

    // Save configuration
    let mut config = agent.config.clone();
    config.llm = llm;
    config.telegram = telegram;
    config.save(config_dir)?;

    println!("═══════════════════════════════════════════════════════════");
    println!("                  SETUP COMPLETE!");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("Configuration saved to: {}", config_dir.display());
    println!();
    println!("To start your agent:");
    println!("  GUI mode:      cogniark");
    println!("  Headless mode: cogniark --headless");
    println!();
    println!("HTTP API will be available at: http://127.0.0.1:17990");
    println!();

    Ok(())
}

async fn run_headless(agent: core::Agent) -> Result<()> {
    tracing::info!("Running in headless mode");

    println!();
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║             CogniArk v{} - Headless Mode               ║", env!("CARGO_PKG_VERSION"));
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();
    println!("DID: {}", agent.identity.did());
    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│  Web UI:   http://127.0.0.1:17990                       │");
    println!("├─────────────────────────────────────────────────────────┤");
    println!("│  API Endpoints:                                         │");
    println!("│    GET  /health  - Health check                         │");
    println!("│    GET  /status  - Agent status                         │");
    println!("│    POST /chat    - Chat with agent                      │");
    println!("│    GET  /actions - List actions                          │");
    println!("│    GET  /tasks   - List tasks                           │");
    println!("└─────────────────────────────────────────────────────────┘");
    println!();
    println!("Press Ctrl+C to stop");
    println!();

    let agent = std::sync::Arc::new(tokio::sync::RwLock::new(agent));

    // Ensure daily brief auto-task exists
    {
        let agent = agent.read().await;
        if let Err(e) = agent.ensure_daily_brief_task().await {
            tracing::warn!("Failed to ensure daily brief task: {}", e);
        }
    }

    // Start HTTP server for local IPC
    let http_handle = {
        let agent = agent.clone();
        tokio::spawn(async move {
            if let Err(e) = channels::http::serve(agent).await {
                tracing::error!("HTTP server error: {}", e);
            }
        })
    };

    // Start Telegram bot if configured
    #[cfg(feature = "telegram")]
    let _telegram_handle = {
        let agent = agent.clone();
        tokio::spawn(async move {
            if let Err(e) = channels::telegram::serve(agent).await {
                tracing::error!("Telegram bot error: {}", e);
            }
        })
    };

    // Background scheduler for due tasks
    let scheduler_handle = {
        let agent = agent.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                let due_tasks = {
                    let agent = agent.read().await;
                    agent.take_due_tasks().await
                };

                for task in due_tasks {
                    let result = {
                        let agent = agent.read().await;
                        agent.execute_task(&task).await
                    };

                    let (status, output) = match result {
                        Ok(out) => (core::TaskStatus::Completed, Some(out)),
                        Err(e) => (core::TaskStatus::Failed { error: e.to_string() }, Some(format!("Error: {}", e))),
                    };

                    {
                        let agent_guard = agent.read().await;
                        let _ = agent_guard.finalize_task(task.id, status, output.clone()).await;

                        let report_to = task.arguments.get("report_to").and_then(|v| v.as_str()).unwrap_or("");
                        #[cfg(feature = "telegram")]
                        {
                            if report_to == "telegram" || (report_to.is_empty() && task.action == "daily_brief") {
                                if let Some(ref text) = output {
                                    let _ = channels::telegram::send_message(&agent_guard, text).await;
                                }
                            }
                        }
                        if report_to == "email" {
                            if let Some(ref text) = output {
                                let email_result = crate::actions::gmail::gmail_profile_email(&agent_guard.config_dir).await;
                                match email_result {
                                    Ok(email) if !email.is_empty() => {
                                        let tz = {
                                            let profile = agent_guard.user_profile.read().await;
                                            profile
                                                .timezone
                                                .as_deref()
                                                .and_then(|value| value.parse::<chrono_tz::Tz>().ok())
                                        };
                                        let date = match tz {
                                            Some(tz) => chrono::Utc::now().with_timezone(&tz).format("%Y-%m-%d").to_string(),
                                            None => chrono::Utc::now().format("%Y-%m-%d").to_string(),
                                        };
                                        let subject = format!("Daily Brief - {}", date);
                                        let email_format = {
                                            let profile = agent_guard.user_profile.read().await;
                                            profile.email_format.clone().unwrap_or_default()
                                        };
                                        let body = match email_format.as_str() {
                                            "narrative" => {
                                                let narrative = text
                                                    .lines()
                                                    .map(|line| line.trim_start_matches("- ").to_string())
                                                    .collect::<Vec<_>>()
                                                    .join(" ");
                                                format!("{}\n\n— {}", narrative, agent_guard.config.name)
                                            }
                                            "sections" => {
                                                format!(
                                                    "Summary\n{}\n\n— {}",
                                                    text,
                                                    agent_guard.config.name
                                                )
                                            }
                                            _ => format!("{}\n\n— {}", text, agent_guard.config.name),
                                        };
                                        let args = serde_json::json!({
                                            "to": email,
                                            "subject": subject,
                                            "body": body
                                        });
                                        let _ = agent_guard.runtime.execute_action("gmail_reply", &args).await;
                                    }
                                    Ok(_) => {
                                        tracing::warn!("Gmail email push skipped: empty email address");
                                    }
                                    Err(e) => {
                                        tracing::warn!("Gmail email push failed: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        })
    };

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    println!();
    tracing::info!("Shutdown signal received");

    http_handle.abort();
    scheduler_handle.abort();

    Ok(())
}
