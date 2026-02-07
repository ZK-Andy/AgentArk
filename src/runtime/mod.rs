//! Action Runtime - WASM Sandbox + Docker + Transactional Execution
//!
//! Based on arXiv:2512.12806 "Fault-Tolerant Sandboxing"
//!
//! Features:
//! - WASM sandbox for lightweight, fast action execution
//! - Docker sandbox for heavier/untrusted operations
//! - Transactional filesystem with rollback capability

mod sandbox;
mod transaction;

pub use sandbox::{SandboxMode, ActionSandbox};
pub use transaction::TransactionManager;

use anyhow::Result;
#[cfg(feature = "docker")]
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::actions::{ActionDef, ActionSource};

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub default_sandbox: SandboxMode,
    pub wasm_memory_limit: u64,
    pub docker_image: String,
    pub enable_rollback: bool,
    pub snapshot_dir: PathBuf,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            default_sandbox: SandboxMode::Wasm,
            wasm_memory_limit: 256 * 1024 * 1024, // 256MB
            docker_image: "cogniark-sandbox:latest".to_string(),
            enable_rollback: true,
            snapshot_dir: PathBuf::from("snapshots"),
        }
    }
}

/// The action runtime that manages execution
pub struct ActionRuntime {
    config: RuntimeConfig,
    #[allow(dead_code)]
    sandbox: ActionSandbox,
    /// Transactions wrapped in Mutex for concurrent access
    transactions: tokio::sync::Mutex<TransactionManager>,
    /// Actions wrapped in RwLock for concurrent access
    actions: tokio::sync::RwLock<HashMap<String, LoadedAction>>,
    actions_dir: PathBuf,
    config_dir: PathBuf,
    /// Shared task queue for list_tasks action
    task_queue: Option<std::sync::Arc<tokio::sync::RwLock<crate::core::TaskQueue>>>,
}

/// A loaded action ready for execution
struct LoadedAction {
    info: ActionDef,
    wasm_module: Option<Vec<u8>>,
    /// Workflow content from ACTION.md (for LLM-driven actions)
    workflow_content: Option<String>,
}

impl ActionRuntime {
    pub async fn new(config_dir: &Path, data_dir: &Path) -> Result<Self> {
        let config_path = config_dir.join("runtime.toml");
        let config: RuntimeConfig = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            toml::from_str(&content)?
        } else {
            let default = RuntimeConfig::default();
            let content = toml::to_string_pretty(&default)?;
            std::fs::write(&config_path, content)?;
            default
        };

        // User actions go in data dir
        let actions_dir = data_dir.join("actions");
        std::fs::create_dir_all(&actions_dir)?;

        let snapshot_dir = data_dir.join(&config.snapshot_dir);
        std::fs::create_dir_all(&snapshot_dir)?;

        let sandbox = ActionSandbox::new(&config)?;
        let transactions = TransactionManager::new(snapshot_dir);

        let runtime = Self {
            config,
            sandbox,
            transactions: tokio::sync::Mutex::new(transactions),
            actions: tokio::sync::RwLock::new(HashMap::new()),
            actions_dir: actions_dir.clone(),
            config_dir: config_dir.to_path_buf(),
            task_queue: None,
        };

        // Load built-in actions
        runtime.load_builtin_actions().await?;

        // Load markdown actions from the app's actions directory (bundled with app)
        // This is /app/actions in Docker, separate from user data
        let app_actions_dir = std::path::Path::new("/app/actions");
        if app_actions_dir.exists() {
            tracing::info!("Loading bundled actions from {:?}", app_actions_dir);
            runtime.load_markdown_actions(app_actions_dir, ActionSource::Bundled).await?;
        }

        // Also check relative actions dir (for local development)
        let local_actions_dir = std::env::current_dir()
            .map(|d| d.join("actions"))
            .unwrap_or_else(|_| std::path::PathBuf::from("./actions"));
        if local_actions_dir.exists() && local_actions_dir != app_actions_dir {
            tracing::info!("Loading local actions from {:?}", local_actions_dir);
            runtime.load_markdown_actions(&local_actions_dir, ActionSource::Bundled).await?;
        }

        // Load user-added actions from data dir
        if actions_dir.exists() {
            tracing::info!("Loading user actions from {:?}", actions_dir);
            runtime.load_markdown_actions(&actions_dir, ActionSource::Custom).await?;
        }

        Ok(runtime)
    }

    /// Set shared task queue reference (called from Agent::init)
    pub fn set_task_queue(&mut self, tasks: std::sync::Arc<tokio::sync::RwLock<crate::core::TaskQueue>>) {
        self.task_queue = Some(tasks);
    }

    /// Load built-in actions
    async fn load_builtin_actions(&self) -> Result<()> {
        // File operations
        self.register_builtin_action(ActionDef {
            name: "file_read".to_string(),
            description: "Read contents of a file".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read" }
                },
                "required": ["path"]
            }),
            capabilities: vec!["file_read".to_string()],
            sandbox_mode: Some(SandboxMode::Native),
            source: ActionSource::System,
            file_path: None,
        }).await;

        self.register_builtin_action(ActionDef {
            name: "file_write".to_string(),
            description: "Write contents to a file".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to write" },
                    "content": { "type": "string", "description": "Content to write" }
                },
                "required": ["path", "content"]
            }),
            capabilities: vec!["file_write".to_string()],
            sandbox_mode: Some(SandboxMode::Native),
            source: ActionSource::System,
            file_path: None,
        }).await;

        // HTTP requests
        self.register_builtin_action(ActionDef {
            name: "http_get".to_string(),
            description: "Make an HTTP GET request".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to fetch" },
                    "headers": { "type": "object", "description": "Optional headers" }
                },
                "required": ["url"]
            }),
            capabilities: vec!["network".to_string()],
            sandbox_mode: Some(SandboxMode::Wasm),
            source: ActionSource::System,
            file_path: None,
        }).await;

        // Shell commands (requires approval by default)
        self.register_builtin_action(ActionDef {
            name: "shell".to_string(),
            description: "Execute a shell command".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Command to execute" },
                    "cwd": { "type": "string", "description": "Working directory" }
                },
                "required": ["command"]
            }),
            capabilities: vec!["shell".to_string()],
            sandbox_mode: Some(SandboxMode::Docker),
            source: ActionSource::System,
            file_path: None,
        }).await;

        // Clipboard
        self.register_builtin_action(ActionDef {
            name: "clipboard_read".to_string(),
            description: "Read from clipboard".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            capabilities: vec!["clipboard_read".to_string()],
            sandbox_mode: Some(SandboxMode::Native),
            source: ActionSource::System,
            file_path: None,
        }).await;

        self.register_builtin_action(ActionDef {
            name: "clipboard_write".to_string(),
            description: "Write to clipboard".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "Content to copy" }
                },
                "required": ["content"]
            }),
            capabilities: vec!["clipboard_write".to_string()],
            sandbox_mode: Some(SandboxMode::Native),
            source: ActionSource::System,
            file_path: None,
        }).await;

        // Scheduler
        self.register_builtin_action(ActionDef {
            name: "schedule_task".to_string(),
            description: "Schedule a recurring or one-time task. Use 'cron' for recurring (e.g., daily at 9am = '0 9 * * *') or 'at' for one-time (ISO timestamp).".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "Task description - what to do" },
                    "cron": { "type": "string", "description": "Cron expression for recurring tasks. Format: 'minute hour day month weekday'. Examples: '0 9 * * *' = daily at 9am, '0 9 * * 1' = every Monday 9am, '*/30 * * * *' = every 30 minutes" },
                    "at": { "type": "string", "description": "ISO 8601 timestamp for one-time task. Example: '2026-02-06T09:00:00+05:30'" }
                },
                "required": ["task"]
            }),
            capabilities: vec!["scheduler".to_string()],
            sandbox_mode: Some(SandboxMode::Native),
            source: ActionSource::System,
            file_path: None,
        }).await;

        // Gmail scan
        self.register_builtin_action(ActionDef {
            name: "gmail_scan".to_string(),
            description: "Read and scan the user's Gmail inbox. Use when asked to check email, find emails, look for meetings/invites/receipts, or anything email-related. Returns sender, subject, date, and labels for each message. Supports Gmail search syntax for filtering.".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Gmail search query (e.g., 'is:unread', 'from:sarah', 'subject:meeting', 'newer_than:2d'). Leave empty for recent inbox." },
                    "labels": { "type": "array", "items": { "type": "string" }, "description": "Label IDs to filter: INBOX (default), SPAM, IMPORTANT, UNREAD, STARRED, SENT, DRAFT, TRASH" },
                    "max_results": { "type": "integer", "description": "Max messages to return (default 10, max 20)" }
                }
            }),
            capabilities: vec!["gmail".to_string()],
            sandbox_mode: Some(SandboxMode::Native),
            source: ActionSource::System,
            file_path: None,
        }).await;

        self.register_builtin_action(ActionDef {
            name: "gmail_reply".to_string(),
            description: "Send an email or reply via the user's Gmail. Use when asked to send, reply to, compose, or draft an email. Can reply to existing threads using thread_id.".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "to": { "type": "string", "description": "Recipient email address" },
                    "subject": { "type": "string", "description": "Email subject line" },
                    "body": { "type": "string", "description": "Email body text (plain text)" },
                    "thread_id": { "type": "string", "description": "Gmail thread ID to reply to (from gmail_scan results)" }
                },
                "required": ["to", "subject", "body"]
            }),
            capabilities: vec!["gmail".to_string()],
            sandbox_mode: Some(SandboxMode::Native),
            source: ActionSource::System,
            file_path: None,
        }).await;

        // Web search
        self.register_builtin_action(ActionDef {
            name: "web_search".to_string(),
            description: "Search the web for current information. Use when asked about news, facts, prices, weather, or anything that needs up-to-date data.".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "num_results": { "type": "integer", "description": "Number of results (default 5)" },
                    "backend": { "type": "string", "description": "Search backend: duckduckgo, brave, serper, searxng" }
                },
                "required": ["query"]
            }),
            capabilities: vec!["network".to_string()],
            sandbox_mode: Some(SandboxMode::Native),
            source: ActionSource::System,
            file_path: None,
        }).await;

        // Research
        self.register_builtin_action(ActionDef {
            name: "research".to_string(),
            description: "Conduct deep research on a topic by searching and analyzing multiple sources. Use for complex questions that need thorough investigation beyond a simple web search.".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Research topic or question" },
                    "max_sources": { "type": "integer", "description": "Maximum sources to examine (default 5)" },
                    "depth": { "type": "string", "description": "Research depth: quick, standard, deep" },
                    "include_sources": { "type": "boolean", "description": "Include source URLs" }
                },
                "required": ["query"]
            }),
            capabilities: vec!["network".to_string()],
            sandbox_mode: Some(SandboxMode::Native),
            source: ActionSource::System,
            file_path: None,
        }).await;

        // List tasks/goals/routines
        self.register_builtin_action(ActionDef {
            name: "list_tasks".to_string(),
            description: "List pending tasks, goals, routines, and scheduled items. Use when the user asks about their pending goals, tasks, agenda, or what's scheduled.".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "filter": { "type": "string", "description": "Filter: 'all', 'pending', 'goals', 'routines', 'completed', 'failed'. Default: 'pending'" }
                }
            }),
            capabilities: vec![],
            sandbox_mode: Some(SandboxMode::Native),
            source: ActionSource::System,
            file_path: None,
        }).await;

        Ok(())
    }

    async fn register_builtin_action(&self, info: ActionDef) {
        self.actions.write().await.insert(
            info.name.clone(),
            LoadedAction {
                info,
                wasm_module: None,
                workflow_content: None,
            },
        );
    }

    /// Register an action with workflow content (from ACTION.md)
    async fn register_workflow_action(&self, info: ActionDef, workflow: String) {
        self.actions.write().await.insert(
            info.name.clone(),
            LoadedAction {
                info,
                wasm_module: None,
                workflow_content: Some(workflow),
            },
        );
    }

    /// Execute an action with given arguments
    pub async fn execute_action(
        &self,
        action_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String> {
        let sandbox_mode = {
            let actions = self.actions.read().await;
            let action = actions
                .get(action_name)
                .ok_or_else(|| anyhow::anyhow!("Unknown action: {}", action_name))?;
            action.info.sandbox_mode.clone().unwrap_or(self.config.default_sandbox.clone())
        };

        // Start transaction if rollback is enabled
        let transaction = if self.config.enable_rollback {
            let mut tx_guard = self.transactions.lock().await;
            Some(tx_guard.begin().await?)
        } else {
            None
        };

        // Execute based on sandbox mode
        let result = match sandbox_mode {
            SandboxMode::Native => self.execute_native(action_name, arguments).await,
            SandboxMode::Wasm => self.execute_wasm(action_name, arguments).await,
            SandboxMode::Docker => self.execute_docker(action_name, arguments).await,
        };

        // Handle transaction
        match (&result, transaction) {
            (Ok(_), Some(tx)) => {
                let mut tx_guard = self.transactions.lock().await;
                tx_guard.commit(tx).await?;
            }
            (Err(_), Some(tx)) => {
                tracing::warn!("Rolling back transaction due to error");
                let mut tx_guard = self.transactions.lock().await;
                tx_guard.rollback(tx).await?;
            }
            _ => {}
        }

        result
    }

    /// Execute an action natively (no sandbox)
    async fn execute_native(
        &self,
        action_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String> {
        match action_name {
            "file_read" => {
                let path = arguments["path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
                let content = tokio::fs::read_to_string(path).await?;
                Ok(content)
            }
            "file_write" => {
                let path = arguments["path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
                let content = arguments["content"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing content"))?;
                tokio::fs::write(path, content).await?;
                Ok(format!("Written to {}", path))
            }
            "clipboard_read" => {
                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| anyhow::anyhow!("Failed to access clipboard: {}", e))?;
                let content = clipboard.get_text()
                    .map_err(|e| anyhow::anyhow!("Failed to read clipboard: {}", e))?;
                Ok(content)
            }
            "clipboard_write" => {
                let content = arguments["content"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing content"))?;
                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| anyhow::anyhow!("Failed to access clipboard: {}", e))?;
                clipboard.set_text(content)
                    .map_err(|e| anyhow::anyhow!("Failed to write clipboard: {}", e))?;
                Ok("Content copied to clipboard".to_string())
            }
            "list_tasks" => {
                let queue = self.task_queue.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Task queue not available"))?;
                let tasks = queue.read().await;
                let filter = arguments.get("filter")
                    .and_then(|v| v.as_str())
                    .unwrap_or("pending");

                let filtered: Vec<_> = tasks.all().iter().filter(|t| match filter {
                    "pending" => matches!(t.status, crate::core::TaskStatus::Pending | crate::core::TaskStatus::AwaitingApproval),
                    "goals" => t.action == "goal",
                    "routines" => t.cron.is_some(),
                    "completed" => matches!(t.status, crate::core::TaskStatus::Completed),
                    "failed" => matches!(t.status, crate::core::TaskStatus::Failed { .. }),
                    _ => true, // "all"
                }).collect();

                if filtered.is_empty() {
                    return Ok(format!("No {} items found.", filter));
                }

                let mut output = format!("Found {} {} item(s):\n\n", filtered.len(), filter);
                for t in &filtered {
                    let status_str = match &t.status {
                        crate::core::TaskStatus::Pending => "Pending",
                        crate::core::TaskStatus::AwaitingApproval => "Awaiting Approval",
                        crate::core::TaskStatus::InProgress => "In Progress",
                        crate::core::TaskStatus::Completed => "Completed",
                        crate::core::TaskStatus::Failed { .. } => "Failed",
                        crate::core::TaskStatus::Cancelled => "Cancelled",
                    };
                    output.push_str(&format!("- {} (status: {})\n", t.description, status_str));
                    if let Some(ref cron) = t.cron {
                        output.push_str(&format!("  Schedule: {}\n", cron));
                    }
                }
                Ok(output)
            }
            "schedule_task" => {
                let task_desc = arguments["task"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing task description"))?;

                let schedule_info = if let Some(cron_expr) = arguments.get("cron").and_then(|v| v.as_str()) {
                    // Auto-convert standard 5-field cron to 6-field (with seconds)
                    // Standard: "minute hour day month weekday" -> "0 9 * * *"
                    // Rust cron: "second minute hour day month weekday" -> "0 0 9 * * *"
                    let cron_6field = if cron_expr.split_whitespace().count() == 5 {
                        format!("0 {}", cron_expr) // Prepend "0 " for seconds
                    } else {
                        cron_expr.to_string()
                    };

                    // Validate cron expression
                    cron_6field.parse::<cron::Schedule>()
                        .map_err(|e| anyhow::anyhow!("Invalid cron expression '{}': {}", cron_6field, e))?;
                    format!("cron:{}", cron_6field)
                } else if let Some(at_time) = arguments.get("at").and_then(|v| v.as_str()) {
                    // Validate ISO timestamp
                    chrono::DateTime::parse_from_rfc3339(at_time)
                        .map_err(|e| anyhow::anyhow!("Invalid timestamp: {}", e))?;
                    format!("at:{}", at_time)
                } else {
                    return Err(anyhow::anyhow!("Must specify either 'cron' or 'at' for scheduling"));
                };

                // Return scheduling info - actual scheduling is handled by the agent's task queue
                Ok(format!("Task scheduled: {} | Schedule: {}", task_desc, schedule_info))
            }
            "gmail_scan" => {
                crate::actions::gmail::gmail_scan(&self.config_dir, arguments).await
            }
            "gmail_reply" => {
                crate::actions::gmail::gmail_reply(&self.config_dir, arguments).await
            }
            "web_search" => {
                let args: crate::actions::search::SearchArgs = serde_json::from_value(arguments.clone())
                    .map_err(|e| anyhow::anyhow!("Invalid search arguments: {}", e))?;

                // Use default config (DuckDuckGo)
                let config = crate::actions::SearchConfig::default();
                crate::actions::search::execute_search(&args, &config).await
            }
            "research" => {
                let args: crate::actions::research::ResearchArgs = serde_json::from_value(arguments.clone())
                    .map_err(|e| anyhow::anyhow!("Invalid research arguments: {}", e))?;

                // Use default config (DuckDuckGo)
                let config = crate::actions::SearchConfig::default();
                crate::actions::research::execute_research(&args, &config).await
            }
            "video-frames" => {
                // This action requires ffmpeg - check arguments
                let video = arguments.get("video")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'video' path argument"))?;
                let time = arguments.get("time")
                    .and_then(|v| v.as_str())
                    .unwrap_or("00:00:00");
                let output = arguments.get("out")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("{}_frame.jpg", video.trim_end_matches(".mp4")));

                // Execute ffmpeg
                let output_result = tokio::process::Command::new("ffmpeg")
                    .args(["-ss", time, "-i", video, "-frames:v", "1", "-q:v", "2", &output, "-y"])
                    .output()
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to run ffmpeg: {}", e))?;

                if output_result.status.success() {
                    Ok(format!("Frame extracted to: {}", output))
                } else {
                    let stderr = String::from_utf8_lossy(&output_result.stderr);
                    Err(anyhow::anyhow!("ffmpeg failed: {}", stderr))
                }
            }
            // Handle workflow actions - return marker for agent to process with LLM
            other => {
                let actions = self.actions.read().await;
                if let Some(action) = actions.get(other) {
                    if action.workflow_content.is_some() {
                        // Return a special marker that tells the agent to use LLM-driven execution
                        let user_query = arguments.get("query")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        return Ok(format!("__WORKFLOW_ACTION__:{}:{}", other, user_query));
                    }
                }
                Err(anyhow::anyhow!("Unknown native action: {}", action_name))
            }
        }
    }

    /// Execute an action in WASM sandbox
    async fn execute_wasm(
        &self,
        action_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String> {
        // For built-in actions, fall back to native with some wrapping
        match action_name {
            "http_get" => {
                let url = arguments["url"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing url"))?;

                let client = reqwest::Client::new();
                let response = client.get(url).send().await?;
                let body = response.text().await?;

                Ok(body)
            }
            _ => {
                // Check if we have a WASM module for this action
                let actions = self.actions.read().await;
                if let Some(action) = actions.get(action_name) {
                    if let Some(wasm_bytes) = &action.wasm_module {
                        let wasm = wasm_bytes.clone();
                        drop(actions); // Release lock before async call
                        return self.run_wasm_module(&wasm, arguments).await;
                    }
                }
                drop(actions); // Release lock before async call
                // Fall back to native
                self.execute_native(action_name, arguments).await
            }
        }
    }

    /// Execute an action in Docker sandbox
    async fn execute_docker(
        &self,
        action_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String> {
        #[cfg(feature = "docker")]
        {
            match action_name {
                "shell" => {
                    let command = arguments["command"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Missing command"))?;

                    let docker = bollard::Docker::connect_with_local_defaults()?;

                    let config = bollard::container::Config {
                        image: Some(self.config.docker_image.clone()),
                        cmd: Some(vec!["sh".to_string(), "-c".to_string(), command.to_string()]),
                        ..Default::default()
                    };

                    let container = docker
                        .create_container::<String, String>(None, config)
                        .await?;

                    docker
                        .start_container::<String>(&container.id, None)
                        .await?;

                    let _output = docker
                        .wait_container::<String>(&container.id, None)
                        .try_collect::<Vec<_>>()
                        .await?;

                    // Get logs
                    let logs = docker
                        .logs::<String>(
                            &container.id,
                            Some(bollard::container::LogsOptions {
                                stdout: true,
                                stderr: true,
                                ..Default::default()
                            }),
                        )
                        .try_collect::<Vec<_>>()
                        .await?;

                    // Clean up
                    docker
                        .remove_container(&container.id, None)
                        .await?;

                    let output_str: String = logs
                        .iter()
                        .map(|l| l.to_string())
                        .collect::<Vec<_>>()
                        .join("");

                    Ok(output_str)
                }
                _ => Err(anyhow::anyhow!("Unknown docker action: {}", action_name)),
            }
        }

        #[cfg(not(feature = "docker"))]
        {
            Err(anyhow::anyhow!(
                "Docker support not enabled. Recompile with --features docker"
            ))
        }
    }

    /// List available actions
    pub async fn list_actions(&self) -> Result<Vec<ActionDef>> {
        let actions = self.actions.read().await;
        Ok(actions.values().map(|s| s.info.clone()).collect())
    }

    /// Get action count
    pub async fn action_count(&self) -> usize {
        self.actions.read().await.len()
    }

    /// Get action info and content for editing
    pub async fn get_action_content(&self, name: &str) -> Result<Option<(ActionDef, String)>> {
        let actions = self.actions.read().await;
        if let Some(action) = actions.get(name) {
            let info = action.info.clone();
            let file_path = action.info.file_path.clone();
            let workflow = action.workflow_content.clone();
            drop(actions); // Release lock before async file I/O

            if let Some(ref fp) = file_path {
                let content = tokio::fs::read_to_string(fp).await?;
                return Ok(Some((info, content)));
            } else if let Some(wf) = workflow {
                return Ok(Some((info, wf)));
            }
            return Ok(Some((info, String::new())));
        }
        Ok(None)
    }

    /// Update action content - for bundled actions, creates a custom copy first
    pub async fn update_action_content(&self, name: &str, content: &str) -> Result<bool> {
        let actions = self.actions.read().await;
        if let Some(action) = actions.get(name) {
            // System actions cannot be edited
            if action.info.source == ActionSource::System {
                return Ok(false);
            }

            // For Bundled actions, create a custom copy in the data directory
            if action.info.source == ActionSource::Bundled {
                drop(actions); // Release lock before async file I/O

                // Create custom action directory
                let custom_action_dir = self.actions_dir.join(name);
                tokio::fs::create_dir_all(&custom_action_dir).await?;

                // Write content to custom location
                let custom_action_file = custom_action_dir.join("ACTION.md");
                tokio::fs::write(&custom_action_file, content).await?;

                tracing::info!("Created custom copy of bundled action '{}' at {:?}", name, custom_action_file);

                // Update the in-memory action to point to the new custom location
                let mut actions = self.actions.write().await;
                if let Some(action) = actions.get_mut(name) {
                    action.info.source = ActionSource::Custom;
                    action.info.file_path = Some(custom_action_file.to_string_lossy().to_string());
                    action.workflow_content = Some(content.to_string());
                }

                return Ok(true);
            }

            // Custom actions - edit in place and update in-memory
            if let Some(ref file_path) = action.info.file_path {
                let fp = file_path.clone();
                drop(actions); // Release lock before async file I/O
                tokio::fs::write(&fp, content).await?;

                // Re-parse and update in-memory action
                if let Ok((new_info, new_content)) = self.parse_action_md(std::path::Path::new(&fp), ActionSource::Custom).await {
                    let mut actions = self.actions.write().await;
                    if let Some(action) = actions.get_mut(name) {
                        action.info.description = new_info.description;
                        action.workflow_content = Some(new_content);
                    }
                }

                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Create a new custom action
    pub async fn create_action(&self, name: &str, content: &str) -> Result<()> {
        let action_dir = self.actions_dir.join(name);
        tokio::fs::create_dir_all(&action_dir).await?;

        let action_file = action_dir.join("ACTION.md");
        tokio::fs::write(&action_file, content).await?;

        // Immediately register into runtime (no restart needed)
        match self.parse_action_md(&action_file, ActionSource::Custom).await {
            Ok((info, workflow_content)) => {
                self.register_workflow_action(info, workflow_content).await;
                tracing::info!("Created and registered action '{}' at {:?}", name, action_file);
            }
            Err(e) => {
                tracing::warn!("Created action file but failed to parse: {}", e);
            }
        }

        Ok(())
    }

    /// Delete a custom action (only Custom actions can be deleted)
    pub async fn delete_action(&self, name: &str) -> Result<bool> {
        let actions = self.actions.read().await;
        if let Some(action) = actions.get(name) {
            // Only Custom actions can be deleted
            if action.info.source != ActionSource::Custom {
                return Ok(false);
            }

            if let Some(ref file_path) = action.info.file_path {
                let action_path = std::path::Path::new(file_path);
                if let Some(action_dir) = action_path.parent() {
                    let dir_path = action_dir.to_path_buf();
                    let action_name_str = name.to_string();
                    drop(actions); // Release lock before async file I/O
                    if dir_path.exists() {
                        tokio::fs::remove_dir_all(&dir_path).await?;
                    }
                    // Remove from in-memory registry
                    let mut actions = self.actions.write().await;
                    actions.remove(&action_name_str);
                    tracing::info!("Deleted action '{}'", action_name_str);
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Check if an action is a workflow action (LLM-driven) and get its workflow content
    /// Returns None if action doesn't exist or has no workflow content
    pub async fn get_workflow_content(&self, action_name: &str) -> Option<String> {
        self.actions
            .read()
            .await
            .get(action_name)
            .and_then(|s| s.workflow_content.clone())
    }

    /// Execute a workflow action with LLM orchestration
    /// This performs web searches based on the workflow, then passes everything to the LLM
    pub async fn execute_workflow_action(
        &self,
        action_name: &str,
        workflow_content: &str,
        user_query: &str,
        llm: &crate::core::LlmClient,
    ) -> Result<String> {
        tracing::info!("Executing LLM-driven workflow action: {}", action_name);

        // Step 1: Extract search queries from the workflow
        let search_queries = self.extract_search_queries(workflow_content, action_name, user_query);

        // Step 2: Perform web searches
        let mut search_results = Vec::new();
        let search_config = crate::actions::SearchConfig::default();

        for query in &search_queries {
            tracing::debug!("Searching: {}", query);
            let args = crate::actions::search::SearchArgs {
                query: query.clone(),
                num_results: 5,
                backend: None,
            };
            match crate::actions::search::execute_search(&args, &search_config).await {
                Ok(results) => {
                    search_results.push(format!("### Search: {}\n{}", query, results));
                }
                Err(e) => {
                    tracing::warn!("Search failed for '{}': {}", query, e);
                    search_results.push(format!("### Search: {} (failed: {})", query, e));
                }
            }
        }

        // Step 3: Build the LLM prompt with workflow instructions and search results
        let combined_results = search_results.join("\n\n");

        let system_prompt = format!(
            r#"You are executing an action workflow. Your task is to analyze the search results and produce output that EXACTLY follows the output format specified in the workflow.

## ACTION WORKFLOW INSTRUCTIONS
{}

## IMPORTANT RULES
1. Follow the "Output Format" section EXACTLY - use the same structure, headings, and formatting
2. Fill in all placeholder sections with actual content based on the search results
3. The LinkedIn post must be 800-1200 characters as specified
4. Include real data, trends, and insights from the search results
5. If search results are insufficient, note this but still produce the best output possible
6. Use today's date where [Date] is specified: {}

## SEARCH RESULTS TO ANALYZE
{}
"#,
            workflow_content,
            chrono::Utc::now().format("%Y-%m-%d"),
            combined_results
        );

        let user_prompt = format!(
            "Execute the workflow above. User's additional context/query: '{}'. Generate the complete output following the exact format specified in the workflow.",
            if user_query.is_empty() { "none" } else { user_query }
        );

        // Step 4: Call LLM to generate the formatted output
        let response = llm.chat(
            &system_prompt,
            &user_prompt,
            &[],  // No memory entries needed
            &[],  // No additional tools
        ).await?;

        Ok(response.content)
    }

    /// Extract search queries from workflow content based on action type
    fn extract_search_queries(&self, workflow: &str, action_name: &str, user_query: &str) -> Vec<String> {
        let mut queries = Vec::new();
        let year = chrono::Utc::now().format("%Y");
        let month = chrono::Utc::now().format("%B");

        // Look for search queries in the workflow (lines starting with - "...")
        for line in workflow.lines() {
            let line = line.trim();
            if line.starts_with("- \"") && line.ends_with("\"") {
                let query = line.trim_start_matches("- \"").trim_end_matches("\"");
                // Replace placeholders
                let query = query
                    .replace("2026", &year.to_string())
                    .replace("February", &month.to_string());
                queries.push(query.to_string());
            }
        }

        // If no queries found in workflow, generate based on action type
        if queries.is_empty() {
            match action_name {
                "trend-prophet" => {
                    queries.push(format!("arxiv AI machine learning latest papers {}", year));
                    queries.push(format!("AI research trends {} emerging", year));
                    queries.push(format!("breakthrough AI papers {} transformer LLM", year));
                    if !user_query.is_empty() {
                        queries.push(format!("AI research {} {}", user_query, year));
                    }
                }
                "market-analysis" => {
                    queries.push(format!("BSE penny stocks India multibagger {}", year));
                    queries.push(format!("Indian stock market analysis small cap {}", year));
                    queries.push(format!("emerging stocks India growth potential {}", year));
                    if !user_query.is_empty() {
                        queries.push(format!("{} stock analysis India {}", user_query, year));
                    }
                }
                _ => {
                    // Generic research queries
                    queries.push(format!("{} latest news {}", action_name, year));
                    queries.push(format!("{} trends analysis {}", action_name, year));
                    if !user_query.is_empty() {
                        queries.push(format!("{} {}", user_query, year));
                    }
                }
            }
        }

        queries
    }

    /// Load markdown-defined actions from a directory
    /// Looks for ACTION.md files in subdirectories
    /// These are registered as workflow actions for LLM-driven execution
    pub async fn load_markdown_actions(&self, dir: &Path, source: ActionSource) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        // Read directory entries
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Could not read actions directory {:?}: {}", dir, e);
                return Ok(());
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let action_md = path.join("ACTION.md");
            if !action_md.exists() { continue; }
            let md_file = action_md;

            match self.parse_action_md(&md_file, source.clone()).await {
                Ok((info, workflow_content)) => {
                    tracing::info!("Loaded workflow action '{}' from {:?}", info.name, md_file);
                    self.register_workflow_action(info, workflow_content).await;
                }
                Err(e) => {
                    tracing::warn!("Failed to load action from {:?}: {}", md_file, e);
                }
            }
        }

        Ok(())
    }

    /// Parse an ACTION.md file to extract action information and full content
    /// Returns (ActionDef, full_workflow_content)
    async fn parse_action_md(&self, path: &Path, source: ActionSource) -> Result<(ActionDef, String)> {
        let content = tokio::fs::read_to_string(path).await?;

        // Parse YAML frontmatter (between --- markers)
        let mut name = String::new();
        let mut description = String::new();
        let mut version = "1.0.0".to_string();

        if content.starts_with("---") {
            if let Some(end_pos) = content[3..].find("---") {
                let frontmatter = &content[3..3 + end_pos];
                for line in frontmatter.lines() {
                    let line = line.trim();
                    if let Some(val) = line.strip_prefix("name:") {
                        name = val.trim().trim_matches('"').to_string();
                    } else if let Some(val) = line.strip_prefix("description:") {
                        description = val.trim().trim_matches('"').to_string();
                    } else if let Some(val) = line.strip_prefix("version:") {
                        version = val.trim().trim_matches('"').to_string();
                    }
                }
            }
        }

        // Fallback: use directory name as action name
        if name.is_empty() {
            name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
        }

        // Extract first heading as description if not in frontmatter
        if description.is_empty() {
            for line in content.lines() {
                if line.starts_with("# ") {
                    description = line[2..].trim().to_string();
                    break;
                }
            }
        }

        let info = ActionDef {
            name,
            description,
            version,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Input/topic for the action" }
                },
                "required": []
            }),
            capabilities: vec!["research".to_string()],
            sandbox_mode: Some(SandboxMode::Native),
            source,
            file_path: Some(path.to_string_lossy().to_string()),
        };

        // Return both the info and the full content for LLM-driven execution
        Ok((info, content))
    }

    /// Load a WASM action from file
    #[allow(dead_code)]
    pub async fn load_wasm_action(&self, path: &Path) -> Result<()> {
        let wasm_bytes = tokio::fs::read(path).await?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid action path"))?
            .to_string();

        // Parse WASM module to extract action info from custom section or use defaults
        let info = self.parse_wasm_action_info(&wasm_bytes, &name, path)?;

        self.actions.write().await.insert(
            name,
            LoadedAction {
                info,
                wasm_module: Some(wasm_bytes),
                workflow_content: None,
            },
        );

        Ok(())
    }

    /// Execute a WASM module with given arguments
    async fn run_wasm_module(
        &self,
        wasm_bytes: &[u8],
        arguments: &serde_json::Value,
    ) -> Result<String> {
        use wasmtime::*;

        // Create engine with config
        let mut config = Config::default();
        config.wasm_component_model(false); // Use core WASM, not component model
        let engine = Engine::new(&config)?;

        // Create a basic store without WASI for simple modules
        let mut store = Store::new(&engine, ());

        // Compile the module
        let module = Module::new(&engine, wasm_bytes)?;

        // Create a linker and instantiate
        let linker = Linker::new(&engine);
        let instance = linker.instantiate(&mut store, &module)?;

        // Try to find entry points
        let result = if let Ok(run_fn) = instance.get_typed_func::<(), ()>(&mut store, "_start") {
            run_fn.call(&mut store, ())?;
            format!("WASM execution completed successfully. Args: {}", serde_json::to_string(arguments)?)
        } else if let Ok(run_fn) = instance.get_typed_func::<(), i32>(&mut store, "run") {
            let exit_code = run_fn.call(&mut store, ())?;
            format!("WASM execution completed with exit code: {}", exit_code)
        } else if let Ok(run_fn) = instance.get_typed_func::<i32, i32>(&mut store, "main") {
            let exit_code = run_fn.call(&mut store, 0)?;
            format!("WASM execution completed with exit code: {}", exit_code)
        } else {
            // List available exports for debugging
            let exports: Vec<String> = instance.exports(&mut store)
                .map(|e| e.name().to_string())
                .collect();
            return Err(anyhow::anyhow!(
                "WASM module has no _start, run, or main entry point. Available exports: {:?}",
                exports
            ));
        };

        Ok(result)
    }

    /// Parse WASM module to extract action info from custom sections
    #[allow(dead_code)]
    fn parse_wasm_action_info(&self, wasm_bytes: &[u8], name: &str, path: &Path) -> Result<ActionDef> {
        // Try to find action metadata in custom section
        // Custom sections in WASM start with a name string, we look for "action_info"
        let mut cursor = 0;
        let mut action_info_json: Option<String> = None;

        while cursor < wasm_bytes.len() {
            // Look for custom section (section id 0)
            if cursor + 1 < wasm_bytes.len() && wasm_bytes[cursor] == 0 {
                cursor += 1;

                // Read section size (LEB128)
                let (section_size, bytes_read) = read_leb128(&wasm_bytes[cursor..]);
                cursor += bytes_read;

                if cursor + section_size as usize <= wasm_bytes.len() {
                    let section_data = &wasm_bytes[cursor..cursor + section_size as usize];

                    // Read section name (LEB128 length + bytes)
                    if !section_data.is_empty() {
                        let (name_len, name_bytes_read) = read_leb128(section_data);
                        if name_bytes_read + name_len as usize <= section_data.len() {
                            let section_name = &section_data[name_bytes_read..name_bytes_read + name_len as usize];
                            if let Ok(name_str) = std::str::from_utf8(section_name) {
                                if name_str == "action_info" {
                                    // Found action_info section, parse the JSON
                                    let json_start = name_bytes_read + name_len as usize;
                                    if let Ok(json_str) = std::str::from_utf8(&section_data[json_start..]) {
                                        action_info_json = Some(json_str.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    cursor += section_size as usize;
                } else {
                    break;
                }
            } else {
                cursor += 1;
            }
        }

        // If we found custom action info, parse it
        if let Some(json) = action_info_json {
            if let Ok(info) = serde_json::from_str::<ActionDef>(&json) {
                return Ok(info);
            }
        }

        // Fall back to default info
        Ok(ActionDef {
            name: name.to_string(),
            description: format!("WASM action from {}", path.display()),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            capabilities: vec!["wasm".to_string()],
            sandbox_mode: Some(SandboxMode::Wasm),
            source: ActionSource::Custom,
            file_path: Some(path.to_string_lossy().to_string()),
        })
    }
}

/// Read an unsigned LEB128 encoded integer
#[allow(dead_code)]
fn read_leb128(bytes: &[u8]) -> (u32, usize) {
    let mut result: u32 = 0;
    let mut shift = 0;
    let mut position = 0;

    for byte in bytes {
        result |= ((byte & 0x7f) as u32) << shift;
        position += 1;

        if byte & 0x80 == 0 {
            break;
        }

        shift += 7;
        if shift >= 32 {
            break;
        }
    }

    (result, position)
}
