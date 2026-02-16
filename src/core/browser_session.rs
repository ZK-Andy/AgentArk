//! Browser session manager for LLM-driven browser automation
//!
//! Each browser session runs as a background tokio task that iteratively:
//! 1. Takes a screenshot + gets page content
//! 2. Calls the LLM with the current state + action history
//! 3. Executes the LLM's chosen action (navigate, click, type, etc.)
//! 4. If the LLM says "ask_user" — sends screenshot to user, pauses, waits for reply
//! 5. Repeats until the task is done or max iterations reached

use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::integrations::browser::BrowserIntegration;

/// Maximum iterations per session before forced stop
const MAX_ITERATIONS: u32 = 30;

/// Browser session status
#[derive(Debug, Clone)]
pub enum SessionStatus {
    /// Session is actively executing
    Active,
    /// Waiting for the user to respond (CAPTCHA, 2FA, etc.)
    WaitingForUser {
        screenshot: Vec<u8>,
        question: String,
    },
    /// Session completed successfully
    Completed { summary: String },
    /// Session failed
    Failed(String),
}

/// A single browser automation session
pub struct BrowserSession {
    pub id: String,
    pub _sidecar_session_id: String,
    pub _channel: String,
    pub chat_id: String,
    pub task_description: String,
    pub status: SessionStatus,
    pub action_history: Vec<String>,
    /// Sender to provide user feedback to the waiting session
    pub user_response_tx: Option<oneshot::Sender<String>>,
}

/// Manages all active browser sessions
pub struct BrowserSessionManager {
    sessions: Arc<DashMap<String, BrowserSession>>,
    integration: Arc<BrowserIntegration>,
}

impl BrowserSessionManager {
    pub fn new() -> Self {
        tracing::debug!("BrowserSessionManager initialized");
        Self {
            sessions: Arc::new(DashMap::new()),
            integration: Arc::new(BrowserIntegration::new()),
        }
    }

    /// Check if the Playwright sidecar is reachable
    pub async fn is_available(&self) -> bool {
        let available = self.integration.is_available().await;
        tracing::debug!("Playwright sidecar availability check: {}", available);
        available
    }

    /// Get the integration reference (for direct calls from execute_tool_calls)
    pub fn integration(&self) -> &Arc<BrowserIntegration> {
        &self.integration
    }

    /// Start a new browser session and spawn the agentic background loop
    pub async fn start_session(
        &self,
        task: &str,
        channel: &str,
        chat_id: &str,
        llm_client: super::llm::LlmClient,
        notify_fn: Arc<dyn Fn(String, Option<Vec<u8>>) + Send + Sync>,
    ) -> Result<String> {
        tracing::info!(
            "Starting browser session: channel={}, task_len={}",
            channel,
            task.len()
        );

        // Create session in sidecar
        let sidecar_id = self.integration.create_session().await?;
        let session_id = uuid::Uuid::new_v4().to_string();
        tracing::info!(
            "Browser session created: session={}, sidecar_session={}",
            &session_id[..8],
            &sidecar_id[..8]
        );

        let session = BrowserSession {
            id: session_id.clone(),
            _sidecar_session_id: sidecar_id.clone(),
            _channel: channel.to_string(),
            chat_id: chat_id.to_string(),
            task_description: task.to_string(),
            status: SessionStatus::Active,
            action_history: Vec::new(),
            user_response_tx: None,
        };

        self.sessions.insert(session_id.clone(), session);

        // Spawn the agentic loop as a background task
        let sessions = self.sessions.clone();
        let integration = self.integration.clone();
        let sid = session_id.clone();
        let task_desc = task.to_string();

        tokio::spawn(async move {
            tracing::info!("Browser agentic loop starting: session={}", &sid[..8]);
            let result = run_browser_loop(
                &sid,
                &sidecar_id,
                &task_desc,
                &sessions,
                &integration,
                &llm_client,
                &notify_fn,
            )
            .await;

            // Cleanup
            tracing::info!("Browser session cleanup: session={}", &sid[..8]);
            let _ = integration.close_session(&sidecar_id).await;
            if let Some(mut entry) = sessions.get_mut(&sid) {
                match &result {
                    Ok(summary) => {
                        tracing::info!(
                            "Browser session completed: session={}, summary_len={}",
                            &sid[..8],
                            summary.len()
                        );
                        entry.status = SessionStatus::Completed {
                            summary: summary.clone(),
                        };
                    }
                    Err(e) => {
                        tracing::error!(
                            "Browser session failed: session={}, error={}",
                            &sid[..8],
                            e
                        );
                        entry.status = SessionStatus::Failed(e.to_string());
                    }
                }
            }
        });

        Ok(session_id)
    }

    /// Provide user feedback to a waiting session
    pub fn provide_user_response(&self, session_id: &str, response: &str) -> bool {
        if let Some(mut entry) = self.sessions.get_mut(session_id) {
            if let Some(tx) = entry.user_response_tx.take() {
                tracing::info!(
                    "User response provided to browser session={}, response_len={}",
                    &session_id[..session_id.len().min(8)],
                    response.len()
                );
                let _ = tx.send(response.to_string());
                entry.status = SessionStatus::Active;
                return true;
            }
        }
        tracing::debug!(
            "No waiting browser session found for response: session={}",
            &session_id[..session_id.len().min(8)]
        );
        false
    }

    /// Check if a user has an active session that's waiting for input
    pub fn get_waiting_session(&self, chat_id: &str) -> Option<(String, Vec<u8>, String)> {
        for entry in self.sessions.iter() {
            // Match by chat_id or match any if chat_id is empty (global check)
            if chat_id.is_empty() || entry.chat_id == chat_id {
                if let SessionStatus::WaitingForUser {
                    ref screenshot,
                    ref question,
                } = entry.status
                {
                    tracing::debug!("Found waiting browser session={}", &entry.id[..8]);
                    return Some((entry.id.clone(), screenshot.clone(), question.clone()));
                }
            }
        }
        None
    }

    /// Get session status
    pub fn get_status(&self, session_id: &str) -> Option<SessionStatus> {
        self.sessions.get(session_id).map(|s| s.status.clone())
    }

    /// List active sessions
    pub fn list_sessions(&self) -> Vec<(String, String, String)> {
        self.sessions
            .iter()
            .map(|entry| {
                let status = match &entry.status {
                    SessionStatus::Active => "active",
                    SessionStatus::WaitingForUser { .. } => "waiting_for_user",
                    SessionStatus::Completed { .. } => "completed",
                    SessionStatus::Failed(_) => "failed",
                };
                (
                    entry.id.clone(),
                    entry.task_description.clone(),
                    status.to_string(),
                )
            })
            .collect()
    }

    /// Count active sessions
    pub fn active_count(&self) -> usize {
        self.sessions
            .iter()
            .filter(|e| {
                matches!(
                    e.status,
                    SessionStatus::Active | SessionStatus::WaitingForUser { .. }
                )
            })
            .count()
    }
}

/// The core agentic browser loop
async fn run_browser_loop(
    session_id: &str,
    sidecar_id: &str,
    task: &str,
    sessions: &Arc<DashMap<String, BrowserSession>>,
    integration: &Arc<BrowserIntegration>,
    llm: &super::llm::LlmClient,
    notify: &Arc<dyn Fn(String, Option<Vec<u8>>) + Send + Sync>,
) -> Result<String> {
    let sid_short = &session_id[..session_id.len().min(8)];

    let browser_system_prompt = format!(
        "You are a browser automation agent. Your task: {}\n\n\
         You control a real web browser. At each step you see:\n\
         - Current page URL and title\n\
         - Page text content (truncated)\n\
         - List of interactive elements (buttons, links, inputs) with their positions\n\n\
         Respond with EXACTLY ONE JSON action per step:\n\
         - {{\"action\": \"navigate\", \"url\": \"...\"}}\n\
         - {{\"action\": \"click\", \"text\": \"...\"}} or {{\"action\": \"click\", \"selector\": \"...\"}} or {{\"action\": \"click\", \"x\": N, \"y\": N}}\n\
         - {{\"action\": \"type_text\", \"text\": \"...\", \"selector\": \"...\"}} (selector optional if field is focused)\n\
         - {{\"action\": \"scroll\", \"direction\": \"down\"}}\n\
         - {{\"action\": \"press_key\", \"key\": \"Enter\"}}\n\
         - {{\"action\": \"ask_user\", \"question\": \"...\"}} — use when stuck (CAPTCHA, 2FA, need credentials, ambiguous choice)\n\
         - {{\"action\": \"done\", \"summary\": \"...\", \"message\": \"...\"}} — task completed. summary is a brief log line, message is what to tell the user\n\
         - {{\"action\": \"notify\", \"message\": \"...\"}} — send a progress update to the user (use sparingly, for key milestones only)\n\n\
         Rules:\n\
         - Always navigate to the target site first\n\
         - After clicking/typing, wait for the next observation before acting again\n\
         - If you encounter a CAPTCHA or 2FA prompt, use ask_user immediately\n\
         - Never guess passwords — use ask_user to request credentials\n\
         - If the page looks wrong after 3 attempts, use ask_user\n\
         - Keep actions simple: one click or one type per step\n\
         - Respond with ONLY the JSON action, no explanation\n\
         - In ask_user and done, write the message naturally as if talking to the user",
        task
    );

    let mut history: Vec<String> = Vec::new();
    tracing::info!(
        "Browser loop starting: session={}, task_len={}",
        sid_short,
        task.len()
    );

    for iteration in 0..MAX_ITERATIONS {
        tracing::debug!(
            "Browser step {}/{}: session={}",
            iteration + 1,
            MAX_ITERATIONS,
            sid_short
        );

        // 1. Get current page state
        let content = match integration.get_content(sidecar_id).await {
            Ok(c) => {
                tracing::debug!(
                    "Page content: session={}, url={}, elements={}",
                    sid_short,
                    &c.url[..c.url.len().min(80)],
                    c.elements.len()
                );
                c
            }
            Err(e) => {
                tracing::warn!(
                    "Browser content fetch error: session={}, error={}",
                    sid_short,
                    e
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        // 2. Build observation string
        let elements_str: String = content
            .elements
            .iter()
            .take(30)
            .map(|e| {
                let label = if !e.text.is_empty() {
                    &e.text
                } else if !e.name.is_empty() {
                    &e.name
                } else if !e.id.is_empty() {
                    &e.id
                } else {
                    "?"
                };
                format!(
                    "[{}] <{}{}> \"{}\" at ({},{}){}",
                    e.index,
                    e.tag,
                    if !e.r#type.is_empty() {
                        format!(" type={}", e.r#type)
                    } else {
                        String::new()
                    },
                    label,
                    e.x,
                    e.y,
                    if !e.href.is_empty() {
                        format!(" href={}", &e.href[..e.href.len().min(60)])
                    } else {
                        String::new()
                    },
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let body_preview = if content.body_text.len() > 2000 {
            format!("{}...", &content.body_text[..2000])
        } else {
            content.body_text.clone()
        };

        let observation = format!(
            "Step {}/{}\nURL: {}\nTitle: {}\n\nPage text:\n{}\n\nInteractive elements:\n{}",
            iteration + 1,
            MAX_ITERATIONS,
            content.url,
            content.title,
            body_preview,
            elements_str
        );

        let mut messages = vec![observation.clone()];
        if !history.is_empty() {
            let recent: Vec<_> = history.iter().rev().take(10).rev().cloned().collect();
            messages.insert(0, format!("Previous actions:\n{}", recent.join("\n")));
        }

        let user_msg = messages.join("\n\n---\n\n");

        // 3. Call LLM for next action
        tracing::debug!(
            "Calling LLM for browser action: session={}, context_len={}",
            sid_short,
            user_msg.len()
        );
        let llm_response = match llm
            .chat_with_system(&browser_system_prompt, &user_msg)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(
                    "Browser LLM call failed: session={}, error={}",
                    sid_short,
                    e
                );
                return Err(e);
            }
        };

        let response_text = llm_response.content.trim().to_string();
        tracing::info!(
            "Browser LLM response: session={}, step={}, response_len={}",
            sid_short,
            iteration + 1,
            response_text.len()
        );

        // 4. Parse the JSON action
        let json_str = if let Some(start) = response_text.find('{') {
            if let Some(end) = response_text.rfind('}') {
                &response_text[start..=end]
            } else {
                &response_text
            }
        } else {
            &response_text
        };

        let action: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "Failed to parse browser action JSON: session={}, error={}, raw_len={}",
                    sid_short,
                    e,
                    response_text.len()
                );
                history.push(format!("Step {}: Error parsing action", iteration + 1));
                continue;
            }
        };

        let action_type = action
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        tracing::info!(
            "Browser action: session={}, step={}, action={}",
            sid_short,
            iteration + 1,
            action_type
        );

        // 5. Execute the action
        match action_type {
            "navigate" => {
                let url = action.get("url").and_then(|v| v.as_str()).unwrap_or("");
                tracing::info!(
                    "Browser navigate: session={}, url_len={}",
                    sid_short,
                    url.len()
                );
                match integration.navigate(sidecar_id, url).await {
                    Ok((final_url, title)) => {
                        tracing::info!(
                            "Browser navigated: session={}, title_len={}",
                            sid_short,
                            title.len()
                        );
                        history.push(format!(
                            "Step {}: Navigated to {} ({})",
                            iteration + 1,
                            final_url,
                            title
                        ));
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Browser navigate failed: session={}, error={}",
                            sid_short,
                            e
                        );
                        history.push(format!("Step {}: Navigate failed: {}", iteration + 1, e));
                    }
                }
            }
            "click" => {
                let selector = action.get("selector").and_then(|v| v.as_str());
                let text = action.get("text").and_then(|v| v.as_str());
                let x = action.get("x").and_then(|v| v.as_i64()).map(|v| v as i32);
                let y = action.get("y").and_then(|v| v.as_i64()).map(|v| v as i32);
                let label = text.or(selector).unwrap_or("element");
                tracing::info!(
                    "Browser click: session={}, target={}",
                    sid_short,
                    &label[..label.len().min(40)]
                );
                match integration.click(sidecar_id, selector, text, x, y).await {
                    Ok(()) => {
                        history.push(format!("Step {}: Clicked '{}'", iteration + 1, label));
                    }
                    Err(e) => {
                        tracing::warn!("Browser click failed: session={}, error={}", sid_short, e);
                        history.push(format!("Step {}: Click failed: {}", iteration + 1, e));
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
            }
            "type_text" => {
                let text = action.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let selector = action.get("selector").and_then(|v| v.as_str());
                let clear = action
                    .get("clear")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                // Log safely — never log typed content (could be passwords)
                tracing::info!(
                    "Browser type: session={}, text_len={}, has_selector={}, clear={}",
                    sid_short,
                    text.len(),
                    selector.is_some(),
                    clear
                );
                match integration
                    .type_text(sidecar_id, text, selector, clear)
                    .await
                {
                    Ok(()) => {
                        history.push(format!(
                            "Step {}: Typed {} chars",
                            iteration + 1,
                            text.len()
                        ));
                    }
                    Err(e) => {
                        tracing::warn!("Browser type failed: session={}, error={}", sid_short, e);
                        history.push(format!("Step {}: Type failed: {}", iteration + 1, e));
                    }
                }
            }
            "scroll" => {
                let dir = action
                    .get("direction")
                    .and_then(|v| v.as_str())
                    .unwrap_or("down");
                let amount = action
                    .get("amount")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32);
                tracing::debug!("Browser scroll: session={}, direction={}", sid_short, dir);
                let _ = integration.scroll(sidecar_id, dir, amount).await;
                history.push(format!("Step {}: Scrolled {}", iteration + 1, dir));
            }
            "press_key" => {
                let key = action
                    .get("key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Enter");
                tracing::debug!("Browser press_key: session={}, key={}", sid_short, key);
                let _ = integration.press_key(sidecar_id, key).await;
                history.push(format!("Step {}: Pressed {}", iteration + 1, key));
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            "notify" => {
                // LLM wants to send a progress update
                let message = action.get("message").and_then(|v| v.as_str()).unwrap_or("");
                if !message.is_empty() {
                    tracing::info!(
                        "Browser notify: session={}, msg_len={}",
                        sid_short,
                        message.len()
                    );
                    notify(message.to_string(), None);
                }
                history.push(format!("Step {}: Sent progress update", iteration + 1));
            }
            "ask_user" => {
                let question = action
                    .get("question")
                    .and_then(|v| v.as_str())
                    .unwrap_or("I need your help to continue.");
                tracing::info!(
                    "Browser ask_user: session={}, question_len={}",
                    sid_short,
                    question.len()
                );

                let screenshot = integration.screenshot(sidecar_id).await.unwrap_or_default();
                tracing::debug!(
                    "Browser screenshot taken: session={}, bytes={}",
                    sid_short,
                    screenshot.len()
                );

                let (tx, rx) = oneshot::channel::<String>();

                if let Some(mut entry) = sessions.get_mut(session_id) {
                    entry.status = SessionStatus::WaitingForUser {
                        screenshot: screenshot.clone(),
                        question: question.to_string(),
                    };
                    entry.user_response_tx = Some(tx);
                }

                notify(question.to_string(), Some(screenshot));

                tracing::info!("Browser waiting for user response: session={}", sid_short);
                match tokio::time::timeout(tokio::time::Duration::from_secs(300), rx).await {
                    Ok(Ok(user_response)) => {
                        tracing::info!(
                            "Browser got user response: session={}, response_len={}",
                            sid_short,
                            user_response.len()
                        );
                        history.push(format!(
                            "Step {}: Asked user, got response ({} chars)",
                            iteration + 1,
                            user_response.len()
                        ));
                        history.push(format!("User replied: {}", user_response));
                    }
                    Ok(Err(_)) => {
                        tracing::warn!(
                            "Browser user response channel closed: session={}",
                            sid_short
                        );
                        return Err(anyhow::anyhow!("User response channel closed"));
                    }
                    Err(_) => {
                        tracing::warn!("Browser user response timeout: session={}", sid_short);
                        notify(
                            "Browser session timed out waiting for your response.".to_string(),
                            None,
                        );
                        return Err(anyhow::anyhow!("Timed out waiting for user response"));
                    }
                }
            }
            "done" => {
                let summary = action
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Task completed");
                let message = action
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or(summary);
                tracing::info!(
                    "Browser session done: session={}, summary_len={}",
                    sid_short,
                    summary.len()
                );

                let screenshot = integration.screenshot(sidecar_id).await.ok();
                notify(message.to_string(), screenshot);

                history.push(format!("Step {}: DONE — {}", iteration + 1, summary));
                return Ok(summary.to_string());
            }
            other => {
                tracing::warn!(
                    "Browser unknown action: session={}, action={}",
                    sid_short,
                    other
                );
                history.push(format!(
                    "Step {}: Unknown action '{}'",
                    iteration + 1,
                    other
                ));
            }
        }

        if let Some(mut entry) = sessions.get_mut(session_id) {
            entry.action_history = history.clone();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    tracing::warn!(
        "Browser session reached max iterations: session={}",
        sid_short
    );
    let screenshot = integration.screenshot(sidecar_id).await.ok();
    notify(
        format!(
            "Browser session reached the maximum of {} steps. Here's where I stopped.",
            MAX_ITERATIONS
        ),
        screenshot,
    );

    Ok(format!("Reached max iterations ({})", MAX_ITERATIONS))
}
