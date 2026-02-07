//! Telegram bot channel

use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{BotCommand, ParseMode};
use tokio::sync::RwLock;

use crate::core::{Agent, Task, TaskApproval, TaskStatus};

type SharedAgent = Arc<RwLock<Agent>>;

/// Split a message into chunks for Telegram (max 4096 chars)
/// Tries to split at paragraph boundaries for better formatting
fn split_message_for_telegram(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for paragraph in text.split("\n\n") {
        let paragraph_with_break = if current_chunk.is_empty() {
            paragraph.to_string()
        } else {
            format!("\n\n{}", paragraph)
        };

        if current_chunk.len() + paragraph_with_break.len() <= max_len {
            current_chunk.push_str(&paragraph_with_break);
        } else {
            // If current paragraph is too long, split it by lines
            if paragraph.len() > max_len {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk);
                    current_chunk = String::new();
                }
                // Split long paragraph by lines
                for line in paragraph.lines() {
                    let line_with_break = if current_chunk.is_empty() {
                        line.to_string()
                    } else {
                        format!("\n{}", line)
                    };

                    if current_chunk.len() + line_with_break.len() <= max_len {
                        current_chunk.push_str(&line_with_break);
                    } else {
                        if !current_chunk.is_empty() {
                            chunks.push(current_chunk);
                        }
                        current_chunk = line.to_string();
                    }
                }
            } else {
                // Start new chunk with this paragraph
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk);
                }
                current_chunk = paragraph.to_string();
            }
        }
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

/// Convert markdown to Telegram HTML format
fn markdown_to_telegram_html(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            // Escape HTML special chars
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '&' => result.push_str("&amp;"),

            // Bold: **text** or __text__
            '*' if chars.peek() == Some(&'*') => {
                chars.next(); // consume second *
                let mut bold_text = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '*' {
                        chars.next();
                        if chars.peek() == Some(&'*') {
                            chars.next();
                            break;
                        }
                        bold_text.push('*');
                    } else {
                        bold_text.push(chars.next().unwrap());
                    }
                }
                result.push_str(&format!("<b>{}</b>", bold_text));
            }

            // Headers: # text -> bold
            '#' if result.ends_with('\n') || result.is_empty() => {
                // Count # symbols
                let mut level = 1;
                while chars.peek() == Some(&'#') {
                    chars.next();
                    level += 1;
                }
                // Skip space after #
                if chars.peek() == Some(&' ') {
                    chars.next();
                }
                // Collect header text until newline
                let mut header_text = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '\n' {
                        break;
                    }
                    header_text.push(chars.next().unwrap());
                }
                // Add emoji prefix for different levels
                let prefix = match level {
                    1 => "📌 ",
                    2 => "▸ ",
                    3 => "• ",
                    _ => "",
                };
                result.push_str(&format!("<b>{}{}</b>", prefix, header_text));
            }

            // Links: [text](url)
            '[' => {
                let mut link_text = String::new();
                let mut found_link = false;
                while let Some(&next) = chars.peek() {
                    if next == ']' {
                        chars.next();
                        if chars.peek() == Some(&'(') {
                            chars.next();
                            let mut url = String::new();
                            while let Some(&url_char) = chars.peek() {
                                if url_char == ')' {
                                    chars.next();
                                    break;
                                }
                                url.push(chars.next().unwrap());
                            }
                            result.push_str(&format!("<a href=\"{}\">{}</a>", url, link_text));
                            found_link = true;
                        }
                        break;
                    }
                    link_text.push(chars.next().unwrap());
                }
                if !found_link {
                    result.push('[');
                    result.push_str(&link_text);
                    result.push(']');
                }
            }

            // Inline code: `code`
            '`' if chars.peek() != Some(&'`') => {
                let mut code_text = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '`' {
                        chars.next();
                        break;
                    }
                    code_text.push(chars.next().unwrap());
                }
                result.push_str(&format!("<code>{}</code>", code_text));
            }

            // Code block: ```code```
            '`' if chars.peek() == Some(&'`') => {
                chars.next(); // second `
                if chars.peek() == Some(&'`') {
                    chars.next(); // third `
                    // Skip optional language identifier
                    while let Some(&next) = chars.peek() {
                        if next == '\n' {
                            chars.next();
                            break;
                        }
                        chars.next();
                    }
                    let mut code_block = String::new();
                    let mut backtick_count = 0;
                    while let Some(&next) = chars.peek() {
                        if next == '`' {
                            backtick_count += 1;
                            chars.next();
                            if backtick_count == 3 {
                                break;
                            }
                        } else {
                            if backtick_count > 0 {
                                for _ in 0..backtick_count {
                                    code_block.push('`');
                                }
                                backtick_count = 0;
                            }
                            code_block.push(chars.next().unwrap());
                        }
                    }
                    result.push_str(&format!("<pre>{}</pre>", code_block.trim()));
                } else {
                    result.push_str("``");
                }
            }

            // Horizontal rule: --- or *** or ___
            '-' if result.ends_with('\n') || result.is_empty() => {
                let mut dash_count = 1;
                while chars.peek() == Some(&'-') {
                    chars.next();
                    dash_count += 1;
                }
                if dash_count >= 3 {
                    result.push_str("─────────────────");
                } else {
                    for _ in 0..dash_count {
                        result.push('-');
                    }
                }
            }

            // Keep everything else
            _ => result.push(c),
        }
    }

    result
}

/// Register bot commands with Telegram (shows in / menu)
async fn register_commands(bot: &Bot) {
    let commands = vec![
        BotCommand::new("help", "Show all commands"),
        BotCommand::new("status", "Agent status"),
        BotCommand::new("image", "Generate an image - /image <prompt>"),
        BotCommand::new("video", "Generate a video - /video <prompt>"),
        BotCommand::new("remind", "Set reminder - /remind <time> <message>"),
        BotCommand::new("weather", "Get weather - /weather [location]"),
        BotCommand::new("translate", "Translate text - /translate <text>"),
        BotCommand::new("summarize", "Summarize our conversation"),
        BotCommand::new("search", "Web search - /search <query>"),
        BotCommand::new("todo", "Manage todo list"),
        BotCommand::new("note", "Save a note - /note <text>"),
        BotCommand::new("tasks", "View pending tasks"),
        BotCommand::new("actions", "List available actions"),
        BotCommand::new("memory", "Memory stats"),
        BotCommand::new("model", "Switch LLM model - /model <name>"),
        BotCommand::new("settings", "View current settings"),
        BotCommand::new("clear", "Clear conversation history"),
    ];

    match bot.set_my_commands(commands).await {
        Ok(_) => tracing::info!("Telegram commands registered successfully"),
        Err(e) => tracing::warn!("Failed to register Telegram commands: {}", e),
    }
}

/// Start the Telegram bot
pub async fn serve(agent: SharedAgent) -> Result<()> {
    let config = {
        let agent = agent.read().await;
        agent.config.telegram.clone()
    };

    let Some(telegram_config) = config else {
        tracing::info!("Telegram not configured, skipping Telegram bot");
        return Ok(());
    };

    tracing::info!("Starting Telegram bot with token: {}...", &telegram_config.bot_token[..8.min(telegram_config.bot_token.len())]);
    if !telegram_config.allowed_users.is_empty() {
        tracing::info!("Telegram allowed users: {:?}", telegram_config.allowed_users);
    } else {
        tracing::info!("Telegram: All users allowed (no restriction)");
    }

    let bot = Bot::new(&telegram_config.bot_token);

    // Register commands with Telegram (shows in / menu)
    register_commands(&bot).await;

    let agent_clone = agent.clone();

    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let agent = agent_clone.clone();
        async move {
            let user_id = msg.from.as_ref().map(|u| u.id.0);
            let chat_id = msg.chat.id;
            tracing::info!("Telegram message received from user {:?} in chat {}", user_id, chat_id);

            if let Some(text) = msg.text() {
                tracing::info!("Telegram message text: {}", text);

                // Check authorization
                let authorized = {
                    let agent = agent.read().await;
                    if let Some(config) = &agent.config.telegram {
                        if config.allowed_users.is_empty() {
                            true
                        } else {
                            msg.from
                                .as_ref()
                                .map(|u| config.allowed_users.contains(&(u.id.0 as i64)))
                                .unwrap_or(false)
                        }
                    } else {
                        true
                    }
                };

                if !authorized {
                    bot.send_message(chat_id, "You are not authorized.")
                        .await?;
                    return Ok(());
                }

                // Persist last chat id for push notifications
                {
                    let agent = agent.read().await;
                    let _ = agent.storage.set(
                        "telegram:last_chat_id",
                        chat_id.0.to_string().as_bytes(),
                    ).await;
                }

                // Handle commands
                if text.starts_with('/') {
                    let response = handle_command(text, &agent).await;
                    bot.send_message(chat_id, response).await?;
                } else {
                    // Process with agent
                    let response = {
                        let mut agent = agent.write().await;
                        match agent.process_message(text, "telegram").await {
                            Ok(r) => r,
                            Err(e) => format!("Error: {}", e),
                        }
                    };

                    // Convert markdown to Telegram HTML
                    let html_response = markdown_to_telegram_html(&response);

                    // Split long messages (Telegram limit is 4096 chars)
                    // Try to split at paragraph boundaries for better formatting
                    let chunks = split_message_for_telegram(&html_response, 4000);
                    for chunk in chunks {
                        bot.send_message(chat_id, chunk)
                            .parse_mode(ParseMode::Html)
                            .await?;
                    }
                }
            }
            Ok(())
        }
    })
    .await;

    Ok(())
}

pub async fn send_message(agent: &Agent, text: &str) -> Result<()> {
    let Some(config) = &agent.config.telegram else {
        return Ok(());
    };

    let chat_id_bytes = agent.storage.get("telegram:last_chat_id").await?;
    let Some(bytes) = chat_id_bytes else {
        return Ok(());
    };
    let chat_id_str = String::from_utf8_lossy(&bytes);
    let chat_id: i64 = chat_id_str.parse().unwrap_or_default();
    if chat_id == 0 {
        return Ok(());
    }

    let bot = Bot::new(&config.bot_token);
    bot.send_message(ChatId(chat_id), text).await?;
    Ok(())
}

async fn handle_command(text: &str, agent: &SharedAgent) -> String {
    let parts: Vec<&str> = text.splitn(2, ' ').collect();
    let command = parts.first().unwrap_or(&"");
    let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match *command {
        "/start" | "/help" => {
            let agent = agent.read().await;
            format!(
                "Welcome to {}! 🤖\n\n\
                📸 Media:\n\
                /image <prompt> - Generate image\n\
                /video <prompt> - Generate video\n\n\
                ⏰ Productivity:\n\
                /remind <time> <msg> - Set reminder\n\
                /todo - View todo list\n\
                /todo add <item> - Add todo\n\
                /note <text> - Save a note\n\
                /tasks - View pending tasks\n\n\
                🔍 Utilities:\n\
                /weather [location] - Get weather\n\
                /translate <text> - Translate\n\
                /search <query> - Web search\n\
                /summarize - Summarize chat\n\n\
                ⚙️ Settings:\n\
                /status - Agent status\n\
                /actions - List actions\n\
                /memory - Memory stats\n\
                /model <name> - Switch model\n\
                /settings - View settings\n\
                /clear - Clear conversation history\n\n\
                Or just chat with me!",
                agent.config.name
            )
        }

        "/status" => {
            let agent = agent.read().await;
            let status = agent.status().await;
            format!(
                "📊 Agent Status\n\n\
                🆔 DID: {}\n\
                🧠 Memory: {} entries\n\
                🛠 Actions: {} loaded\n\
                📋 Tasks: {} pending",
                status.did, status.memory_entries, status.actions_loaded, status.tasks_pending
            )
        }

        "/settings" => {
            let agent = agent.read().await;
            let model = match &agent.config.llm {
                crate::core::LlmProvider::Ollama { model, .. } => format!("Ollama: {}", model),
                crate::core::LlmProvider::Anthropic { model, .. } => format!("Anthropic: {}", model),
                crate::core::LlmProvider::OpenAI { model, .. } => format!("OpenAI: {}", model),
            };
            let fallback = agent.config.llm_fallback.as_ref().map(|fb| {
                match fb {
                    crate::core::LlmProvider::Ollama { model, .. } => format!("Ollama: {}", model),
                    crate::core::LlmProvider::Anthropic { model, .. } => format!("Anthropic: {}", model),
                    crate::core::LlmProvider::OpenAI { model, .. } => format!("OpenAI: {}", model),
                }
            }).unwrap_or_else(|| "None".to_string());

            format!(
                "⚙️ Current Settings\n\n\
                🤖 Bot: {}\n\
                💬 Personality: {}\n\
                🧠 Model: {}\n\
                🔄 Fallback: {}",
                agent.config.name,
                agent.config.personality,
                model,
                fallback
            )
        }

        "/actions" | "/action" => {
            let agent = agent.read().await;
            let actions = agent.runtime.list_actions().await.unwrap_or_default();
            if actions.is_empty() {
                "No actions loaded".to_string()
            } else {
                let list = actions
                    .iter()
                    .take(15) // Limit to prevent too long message
                    .map(|s| format!("• {} - {}", s.name, s.description))
                    .collect::<Vec<_>>()
                    .join("\n");
                let more = if actions.len() > 15 {
                    format!("\n\n...and {} more", actions.len() - 15)
                } else {
                    String::new()
                };
                format!("🛠 Available Actions:\n\n{}{}", list, more)
            }
        }

        "/memory" => {
            let agent = agent.read().await;
            let status = agent.status().await;
            format!(
                "🧠 Memory Stats\n\n\
                📝 Entries: {}\n\
                🛠 Actions: {}\n\
                📋 Tasks: {}",
                status.memory_entries, status.actions_loaded, status.tasks_pending
            )
        }

        "/image" => {
            if args.is_empty() {
                "Usage: /image <prompt>\n\nExample: /image a cute robot playing guitar".to_string()
            } else {
                // Process through agent with image generation intent
                let response = {
                    let mut agent = agent.write().await;
                    let prompt = format!("Generate an image of: {}", args);
                    match agent.process_message(&prompt, "telegram").await {
                        Ok(r) => r,
                        Err(e) => format!("❌ Error: {}", e),
                    }
                };
                response
            }
        }

        "/video" => {
            if args.is_empty() {
                "Usage: /video <prompt>\n\nExample: /video a rocket launching into space".to_string()
            } else {
                let response = {
                    let mut agent = agent.write().await;
                    let prompt = format!("Generate a video of: {}", args);
                    match agent.process_message(&prompt, "telegram").await {
                        Ok(r) => r,
                        Err(e) => format!("❌ Error: {}", e),
                    }
                };
                response
            }
        }

        "/remind" => {
            if args.is_empty() {
                "Usage: /remind <time> <message>\n\nExamples:\n/remind 5m Check the oven\n/remind 2h Call mom\n/remind tomorrow 9am Meeting".to_string()
            } else {
                let response = {
                    let mut agent = agent.write().await;
                    let prompt = format!("Set a reminder: {}", args);
                    match agent.process_message(&prompt, "telegram").await {
                        Ok(r) => r,
                        Err(e) => format!("❌ Error: {}", e),
                    }
                };
                response
            }
        }

        "/weather" => {
            let location = if args.is_empty() { "my location" } else { args };
            let response = {
                let mut agent = agent.write().await;
                let prompt = format!("What's the weather in {}?", location);
                match agent.process_message(&prompt, "telegram").await {
                    Ok(r) => r,
                    Err(e) => format!("❌ Error: {}", e),
                }
            };
            response
        }

        "/translate" => {
            if args.is_empty() {
                "Usage: /translate <text>\n\nExample: /translate Hello, how are you? to Spanish".to_string()
            } else {
                let response = {
                    let mut agent = agent.write().await;
                    let prompt = format!("Translate: {}", args);
                    match agent.process_message(&prompt, "telegram").await {
                        Ok(r) => r,
                        Err(e) => format!("❌ Error: {}", e),
                    }
                };
                response
            }
        }

        "/search" => {
            if args.is_empty() {
                "Usage: /search <query>\n\nExample: /search latest news about AI".to_string()
            } else {
                let response = {
                    let mut agent = agent.write().await;
                    let prompt = format!("Search the web for: {}", args);
                    match agent.process_message(&prompt, "telegram").await {
                        Ok(r) => r,
                        Err(e) => format!("❌ Error: {}", e),
                    }
                };
                response
            }
        }

        "/summarize" => {
            let response = {
                let mut agent = agent.write().await;
                match agent.process_message("Summarize our recent conversation", "telegram").await {
                    Ok(r) => r,
                    Err(e) => format!("❌ Error: {}", e),
                }
            };
            response
        }

        "/todo" => {
            if args.is_empty() {
                // Show todo list
                let response = {
                    let mut agent = agent.write().await;
                    match agent.process_message("Show my todo list", "telegram").await {
                        Ok(r) => r,
                        Err(e) => format!("❌ Error: {}", e),
                    }
                };
                response
            } else if args.starts_with("add ") {
                let item = args.strip_prefix("add ").unwrap_or("").trim();
                let response = {
                    let mut agent = agent.write().await;
                    let prompt = format!("Add to my todo list: {}", item);
                    match agent.process_message(&prompt, "telegram").await {
                        Ok(r) => r,
                        Err(e) => format!("❌ Error: {}", e),
                    }
                };
                response
            } else {
                "Usage:\n/todo - Show list\n/todo add <item> - Add item".to_string()
            }
        }

        "/note" => {
            if args.is_empty() {
                "Usage: /note <text>\n\nExample: /note Remember to buy milk".to_string()
            } else {
                let response = {
                    let mut agent = agent.write().await;
                    let prompt = format!("Save this note: {}", args);
                    match agent.process_message(&prompt, "telegram").await {
                        Ok(r) => r,
                        Err(e) => format!("❌ Error: {}", e),
                    }
                };
                response
            }
        }

        "/tasks" => {
            let agent = agent.read().await;
            let tasks = agent.tasks.read().await;
            let pending: Vec<_> = tasks.all().iter()
                .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::AwaitingApproval))
                .take(10)
                .collect();

            if pending.is_empty() {
                "📋 No pending tasks".to_string()
            } else {
                let list = pending.iter()
                    .map(|t| {
                        let status = match t.status {
                            TaskStatus::AwaitingApproval => "⏳",
                            TaskStatus::Pending => "📌",
                            _ => "•",
                        };
                        format!("{} {}", status, t.description)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("📋 Pending Tasks:\n\n{}", list)
            }
        }

        "/clear" => {
            let agent = agent.read().await;
            agent.clear_conversation_history("telegram").await;
            "🧹 Conversation cleared! Starting fresh.".to_string()
        }

        "/model" => {
            if args.is_empty() {
                let agent = agent.read().await;
                let current = match &agent.config.llm {
                    crate::core::LlmProvider::Ollama { model, .. } => model.clone(),
                    crate::core::LlmProvider::Anthropic { model, .. } => model.clone(),
                    crate::core::LlmProvider::OpenAI { model, .. } => model.clone(),
                };
                format!("Current model: {}\n\nUsage: /model <model_name>\n\nNote: Changing models requires restart", current)
            } else {
                format!("Model change to '{}' noted.\n\n⚠️ To apply, please update via web UI settings and restart.", args)
            }
        }

        cmd if cmd.starts_with("/run ") => {
            let action_name = args;
            if action_name.is_empty() {
                "Usage: /run <action_name>".to_string()
            } else {
                let agent = agent.read().await;
                let actions = agent.runtime.list_actions().await.unwrap_or_default();
                if actions.iter().any(|s| s.name == action_name) {
                    format!("Running action: {}\n\nSend your query for this action.", action_name)
                } else {
                    format!("Action '{}' not found. Use /actions to see available.", action_name)
                }
            }
        }

        cmd if cmd.starts_with("/task ") => {
            let description = args;
            if description.is_empty() {
                "Usage: /task <description>".to_string()
            } else {
                let task = Task {
                    id: uuid::Uuid::new_v4(),
                    description: description.to_string(),
                    action: "telegram".to_string(),
                    arguments: serde_json::json!({ "description": description }),
                    approval: TaskApproval::Auto,
                    capabilities: vec!["telegram".to_string()],
                    status: TaskStatus::Pending,
                    created_at: chrono::Utc::now(),
                    scheduled_for: None,
                    cron: None,
                    result: None,
                    proof_id: None,
                };

                let add_result = {
                    let agent = agent.read().await;
                    agent.add_task(task).await
                };

                match add_result {
                    Ok(_) => format!("✅ Task created: {}", description),
                    Err(e) => format!("❌ Failed to create task: {}", e),
                }
            }
        }

        _ => format!("Unknown command: {}\n\nType /help for all commands", command),
    }
}
