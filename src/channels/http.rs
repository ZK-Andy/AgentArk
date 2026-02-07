//! Local HTTP API for IPC (no WebSockets - localhost only)

use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::{Agent, LlmProvider, ExecutionTrace, TaskQueue, UserProfile, TaskStatus};
use crate::core::config::TelegramConfig;
use super::web::WEB_UI_HTML;

type SharedAgent = Arc<RwLock<Agent>>;

/// Shared application state - allows accessing some data without locking the agent
#[derive(Clone)]
pub struct AppState {
    /// Full agent (requires lock for most operations)
    pub agent: SharedAgent,
    /// Trace history - can be read without locking agent
    pub trace_history: Arc<RwLock<Vec<ExecutionTrace>>>,
    /// Current trace - can be read without locking agent
    pub last_trace: Arc<RwLock<ExecutionTrace>>,
    /// Task queue - can be read without locking agent
    pub tasks: Arc<RwLock<TaskQueue>>,
    /// User profile - can be read without locking agent
    pub user_profile: Arc<RwLock<UserProfile>>,
}

/// Chat request
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default = "default_channel")]
    pub channel: String,
}

fn default_channel() -> String {
    "http".to_string()
}

/// Chat response
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub response: String,
    pub proof_id: Option<String>,
}

/// Agent status response
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub did: String,
    pub memory_entries: usize,
    pub actions_loaded: usize,
    pub tasks_pending: usize,
    pub version: String,
}

/// Actions response
#[derive(Debug, Serialize)]
pub struct ActionsResponse {
    pub actions: Vec<ActionInfo>,
}

#[derive(Debug, Serialize)]
pub struct ActionInfo {
    pub name: String,
    pub description: String,
    pub version: String,
    pub source: String,
    pub editable: bool,
    pub file_path: Option<String>,
}

/// Action content response
#[derive(Debug, Serialize)]
pub struct ActionContentResponse {
    pub name: String,
    pub content: String,
    pub editable: bool,
}

/// Action content update request
#[derive(Debug, Deserialize)]
pub struct ActionContentUpdate {
    pub content: String,
}

/// Create action request
#[derive(Debug, Deserialize)]
pub struct CreateActionRequest {
    pub name: String,
    pub content: String,
}

/// User profile response
#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    pub name: Option<String>,
    pub location: Option<String>,
    pub timezone: Option<String>,
    pub language: Option<String>,
    pub tone: Option<String>,
    pub email_format: Option<String>,
    pub preferences: Option<String>,
    pub onboarding_complete: bool,
}

/// Tasks response
#[derive(Debug, Serialize)]
pub struct TasksResponse {
    pub tasks: Vec<TaskInfo>,
}

#[derive(Debug, Serialize)]
pub struct TaskInfo {
    pub id: String,
    pub description: String,
    pub action: String,
    pub arguments: serde_json::Value,
    pub status: String,
    pub cron: Option<String>,
    pub result: Option<String>,
    pub created_at: String,
}

/// Create task request
#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub description: String,
    pub action: String,
    pub arguments: serde_json::Value,
    /// Cron expression for scheduling (e.g., "*/5 * * * *" for every 5 minutes)
    pub cron: Option<String>,
    /// Approval policy: "auto" or "require"
    pub approval: Option<String>,
}

/// Update task request
#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub description: Option<String>,
    pub arguments: Option<serde_json::Value>,
    pub cron: Option<String>,
}

/// Plan task request (LLM-assisted)
#[derive(Debug, Deserialize)]
pub struct PlanTaskRequest {
    pub description: String,
    pub prompt: Option<String>,
}

/// Plan task response
#[derive(Debug, Serialize)]
pub struct PlanTaskResponse {
    pub plan: serde_json::Value,
}

/// Gmail OAuth start response (Authorization Code flow)
#[derive(Debug, Serialize)]
pub struct GmailOAuthStartResponse {
    pub auth_url: String,
}

/// Settings response (for GET)
#[derive(Debug, Serialize)]
pub struct SettingsResponse {
    pub bot_name: String,
    pub personality: String,
    pub timezone: Option<String>,
    pub language: Option<String>,
    pub tone: Option<String>,
    pub email_format: Option<String>,
    pub daily_brief_channel: String,
    // Primary LLM
    pub llm_provider: String,
    pub llm_model: String,
    pub llm_base_url: Option<String>,
    pub has_api_key: bool,
    // Fallback LLM
    pub llm_fallback_provider: Option<String>,
    pub llm_fallback_model: Option<String>,
    pub llm_fallback_base_url: Option<String>,
    pub has_fallback_api_key: bool,
    // Telegram
    pub telegram_enabled: bool,
    pub has_telegram_token: bool,
    pub telegram_allowed_users: Vec<i64>,
    pub settings_complete: bool,
}

/// Settings update request (for POST)
#[derive(Debug, Deserialize)]
pub struct SettingsUpdate {
    pub bot_name: Option<String>,
    pub personality: Option<String>,
    pub timezone: Option<String>,
    pub language: Option<String>,
    pub tone: Option<String>,
    pub email_format: Option<String>,
    pub daily_brief_channel: Option<String>,
    // Primary LLM
    pub llm_provider: String,
    pub llm_model: String,
    pub llm_base_url: Option<String>,
    pub llm_api_key: Option<String>,
    // Fallback LLM (used if primary fails)
    pub llm_fallback_provider: Option<String>,
    pub llm_fallback_model: Option<String>,
    pub llm_fallback_base_url: Option<String>,
    pub llm_fallback_api_key: Option<String>,
    // Telegram
    pub telegram_enabled: bool,
    pub telegram_bot_token: Option<String>,
    pub telegram_allowed_users: Option<Vec<i64>>,
    /// Media generation provider API keys (all stored encrypted)
    #[serde(default)]
    pub media_providers: std::collections::HashMap<String, String>,
    /// Default provider for image generation
    pub default_image_provider: Option<String>,
    /// Fallback provider for image generation
    pub fallback_image_provider: Option<String>,
    /// Default provider for video generation
    pub default_video_provider: Option<String>,
    /// Fallback provider for video generation
    pub fallback_video_provider: Option<String>,
}

/// Media settings response
#[derive(Debug, Serialize)]
pub struct MediaSettingsResponse {
    pub configured: Vec<String>,
    pub default_image_provider: Option<String>,
    pub fallback_image_provider: Option<String>,
    pub default_video_provider: Option<String>,
    pub fallback_video_provider: Option<String>,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Trace step for execution visibility
#[derive(Debug, Serialize)]
pub struct TraceStep {
    pub icon: String,
    pub title: String,
    pub detail: String,
    #[serde(rename = "type")]
    pub step_type: String,
    pub data: Option<String>,
    pub time: String,
}

/// Execution proof summary
#[derive(Debug, Serialize)]
pub struct ProofSummary {
    pub id: String,
    pub message_preview: String,
    pub time: String,
}

/// Trace response (returns trace history for list view)
#[derive(Debug, Serialize)]
pub struct TraceResponse {
    pub trace: Vec<TraceStep>,
    pub proofs: Vec<ProofSummary>,
    /// List of recent trace summaries (for sidebar/list view)
    pub history: Vec<TraceSummary>,
}

/// Summary of a trace for list view
#[derive(Debug, Serialize)]
pub struct TraceSummary {
    pub id: String,
    pub message_preview: String,
    pub channel: String,
    pub status: String,
    pub step_count: usize,
    pub started_at: String,
    pub duration_ms: Option<u64>,
}

/// Single trace detail response
#[derive(Debug, Serialize)]
pub struct TraceDetailResponse {
    pub id: String,
    pub message: String,
    pub channel: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub steps: Vec<TraceStep>,
    pub response: Option<String>,
    pub proof_id: Option<String>,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}

/// Start the HTTP server (localhost only)
pub async fn serve(agent: SharedAgent) -> Result<()> {
    // Clone Arc handles for independent access (avoids blocking during long operations)
    let state = {
        let agent_guard = agent.read().await;
        AppState {
            agent: agent.clone(),
            trace_history: agent_guard.trace_history.clone(),
            last_trace: agent_guard.last_trace.clone(),
            tasks: agent_guard.tasks.clone(),
            user_profile: agent_guard.user_profile.clone(),
        }
    };

    let app = Router::new()
        // Web UI routes
        .route("/", get(web_ui))
        .route("/ui", get(web_ui))
        .route("/logo.svg", get(serve_logo_svg))
        .route("/logo.png", get(serve_logo_png))
        .route("/logo.jpg", get(serve_logo_jpg))
        // API routes
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/chat", post(chat))
        .route("/chat/clear", post(clear_chat))
        .route("/actions", get(list_actions))
        .route("/actions", post(create_action))
        .route("/actions/{name}", get(get_action_content))
        .route("/actions/{name}", post(update_action_content))
        .route("/actions/{name}", axum::routing::delete(delete_action))
        .route("/tasks", get(list_tasks))
        .route("/tasks", post(create_task))
        .route("/tasks/plan", post(plan_task))
        .route("/tasks/{id}", post(update_task))
        .route("/tasks/{id}", axum::routing::delete(delete_task))
        .route("/tasks/{id}/approve", post(approve_task))
        .route("/tasks/{id}/reject", post(reject_task))
        .route("/goals", get(list_goals))
        .route("/goals", post(create_goal))
        .route("/goals/{id}", axum::routing::delete(delete_goal_endpoint))
        .route("/gmail/oauth/start", post(gmail_oauth_start))
        .route("/gmail/status", get(gmail_status))
        .route("/gmail/test", get(gmail_test))
        .route("/settings", get(get_settings))
        .route("/settings", post(update_settings))
        .route("/settings/media", get(get_media_settings))
        .route("/profile", get(get_profile))
        .route("/restart", post(restart_server))
        .route("/trace", get(get_trace))
        .route("/trace/{id}", get(get_trace_detail))
        // OAuth and integrations routes
        .route("/oauth/callback", get(oauth_callback))
        .route("/integrations", get(list_integrations))
        .route("/integrations/{id}/auth", get(get_integration_auth_url))
        .route("/integrations/{id}/disconnect", post(disconnect_integration))
        .route("/integrations/{id}/configure", post(configure_integration))
        .route("/gmail/configure", post(configure_gmail))
        .with_state(state);

    // Bind to all interfaces (needed for Docker)
    // For production, use a reverse proxy or firewall for security
    let bind_addr = std::env::var("COGNIARK_BIND").unwrap_or_else(|_| "0.0.0.0:17990".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("HTTP server listening on http://127.0.0.1:17990");
    tracing::info!("Web UI available at http://127.0.0.1:17990/");

    axum::serve(listener, app).await?;

    Ok(())
}

/// Serve the embedded web UI
async fn web_ui() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        Html(WEB_UI_HTML),
    )
}

/// Serve PNG logo
async fn serve_logo_png() -> Response {
    // Try to include PNG at compile time, return 404 if not available
    match option_env!("LOGO_PNG_EXISTS") {
        _ => {
            // Try to read from filesystem at runtime as fallback
            if let Ok(bytes) = std::fs::read("assets/logo.png") {
                return ([(header::CONTENT_TYPE, "image/png")], bytes).into_response();
            }
            // Check common paths
            for path in &["/app/assets/logo.png", "./assets/logo.png", "../assets/logo.png"] {
                if let Ok(bytes) = std::fs::read(path) {
                    return ([(header::CONTENT_TYPE, "image/png")], bytes).into_response();
                }
            }
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

/// Serve JPG logo
async fn serve_logo_jpg() -> Response {
    // Try to read from filesystem at runtime
    if let Ok(bytes) = std::fs::read("assets/logo.jpg") {
        return ([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response();
    }
    // Check common paths
    for path in &["/app/assets/logo.jpg", "./assets/logo.jpg", "../assets/logo.jpg"] {
        if let Ok(bytes) = std::fs::read(path) {
            return ([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response();
        }
    }
    StatusCode::NOT_FOUND.into_response()
}

/// Serve SVG logo (animated)
async fn serve_logo_svg() -> Response {
    // Try to read from filesystem at runtime
    if let Ok(bytes) = std::fs::read("assets/logo.svg") {
        return ([(header::CONTENT_TYPE, "image/svg+xml")], bytes).into_response();
    }
    // Check common paths
    for path in &["/app/assets/logo.svg", "./assets/logo.svg", "../assets/logo.svg"] {
        if let Ok(bytes) = std::fs::read(path) {
            return ([(header::CONTENT_TYPE, "image/svg+xml")], bytes).into_response();
        }
    }
    StatusCode::NOT_FOUND.into_response()
}

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

/// Get agent status
async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let agent = state.agent.read().await;
    let status = agent.status().await;

    Json(StatusResponse {
        did: status.did,
        memory_entries: status.memory_entries,
        actions_loaded: status.actions_loaded,
        tasks_pending: status.tasks_pending,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Get user profile (for checking onboarding status)
async fn get_profile(State(state): State<AppState>) -> Json<ProfileResponse> {
    let profile = state.user_profile.read().await;
    Json(ProfileResponse {
        name: profile.name.clone(),
        location: profile.location.clone(),
        timezone: profile.timezone.clone(),
        language: profile.language.clone(),
        tone: profile.tone.clone(),
        email_format: profile.email_format.clone(),
        preferences: profile.preferences.clone(),
        onboarding_complete: profile.onboarding_complete,
    })
}

/// Chat with the agent
async fn chat(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> Response {
    let result = {
        let mut agent_guard = state.agent.write().await;
        agent_guard.process_message(&request.message, &request.channel).await
    };

    match result {
        Ok(response) => (
            StatusCode::OK,
            Json(ChatResponse {
                response,
                proof_id: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Clear conversation history for a channel
async fn clear_chat(
    State(state): State<AppState>,
    Json(request): Json<serde_json::Value>,
) -> Response {
    let channel = request.get("channel")
        .and_then(|v| v.as_str())
        .unwrap_or("web");
    let agent = state.agent.read().await;
    agent.clear_conversation_history(channel).await;
    (StatusCode::OK, Json(serde_json::json!({ "status": "cleared" }))).into_response()
}

/// List available actions
async fn list_actions(State(state): State<AppState>) -> Response {
    let result = {
        let agent_guard = state.agent.read().await;
        agent_guard.runtime.list_actions().await
    };

    match result {
        Ok(actions) => {
            let action_infos: Vec<ActionInfo> = actions
                .into_iter()
                .map(|s| {
                    use crate::actions::ActionSource;
                    let source_str = match &s.source {
                        ActionSource::System => "system",
                        ActionSource::Bundled => "bundled",
                        ActionSource::Custom => "custom",
                    };
                    // Custom and Bundled actions are editable (Bundled gets copied to custom on edit)
                    // Only System actions are read-only
                    let editable = s.source != ActionSource::System;
                    ActionInfo {
                        name: s.name,
                        description: s.description,
                        version: s.version,
                        source: source_str.to_string(),
                        editable,
                        file_path: s.file_path,
                    }
                })
                .collect();

            (StatusCode::OK, Json(ActionsResponse { actions: action_infos })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Get action content (for editing)
async fn get_action_content(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    let agent_guard = state.agent.read().await;

    // Get action info and content from runtime
    match agent_guard.runtime.get_action_content(&name).await {
        Ok(Some((info, content))) => {
            use crate::actions::ActionSource;
            // Custom and Bundled actions are editable (Bundled gets copied to custom on edit)
            let editable = info.source != ActionSource::System;
            (StatusCode::OK, Json(ActionContentResponse {
                name: info.name,
                content,
                editable,
            })).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Action '{}' not found", name),
            }),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ).into_response(),
    }
}

/// Update action content
async fn update_action_content(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(update): Json<ActionContentUpdate>,
) -> Response {
    let agent_guard = state.agent.read().await;

    // Check if action exists and is editable
    match agent_guard.runtime.update_action_content(&name, &update.content).await {
        Ok(true) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "message": "Action updated"})),
        ).into_response(),
        Ok(false) => (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Action is not editable (system action)".to_string(),
            }),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ).into_response(),
    }
}

/// Create a new action
async fn create_action(
    State(state): State<AppState>,
    Json(request): Json<CreateActionRequest>,
) -> Response {
    // Validate action name
    if !request.name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Action name must contain only lowercase letters, numbers, and hyphens".to_string(),
            }),
        ).into_response();
    }

    let agent_guard = state.agent.read().await;

    match agent_guard.runtime.create_action(&request.name, &request.content).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "message": "Action created"})),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ).into_response(),
    }
}

/// Delete an action
async fn delete_action(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    let agent_guard = state.agent.read().await;

    match agent_guard.runtime.delete_action(&name).await {
        Ok(true) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "message": "Action deleted"})),
        ).into_response(),
        Ok(false) => (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Action cannot be deleted (system action or not found)".to_string(),
            }),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ).into_response(),
    }
}

/// List tasks - uses independent Arc, doesn't block during long operations
async fn list_tasks(State(state): State<AppState>) -> Json<TasksResponse> {
    // Access tasks directly without locking agent
    let tasks = state.tasks.read().await;

    let task_infos: Vec<TaskInfo> = tasks
        .all()
        .iter()
        .map(|t| TaskInfo {
            id: t.id.to_string(),
            description: t.description.clone(),
            action: t.action.clone(),
            arguments: t.arguments.clone(),
            status: format!("{:?}", t.status),
            cron: t.cron.clone(),
            result: t.result.clone(),
            created_at: t.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
        .collect();

    Json(TasksResponse { tasks: task_infos })
}

// =============================================================================
// Goals API (goals are stored as tasks with action="goal")
// =============================================================================

/// List goals
async fn list_goals(State(state): State<AppState>) -> Response {
    let tasks = state.tasks.read().await;
    let goals: Vec<serde_json::Value> = tasks.all().iter()
        .filter(|t| t.action == "goal")
        .map(|t| serde_json::json!({
            "id": t.id.to_string(),
            "description": t.description,
            "created_at": t.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        }))
        .collect();
    (StatusCode::OK, Json(serde_json::json!({ "goals": goals }))).into_response()
}

/// Create a goal
async fn create_goal(
    State(state): State<AppState>,
    Json(request): Json<serde_json::Value>,
) -> Response {
    let description = match request.get("description").and_then(|v| v.as_str()) {
        Some(d) if !d.trim().is_empty() => d.trim().to_string(),
        _ => return (StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Missing or empty description".to_string() })).into_response(),
    };

    let task = crate::core::Task::new(description, "goal".to_string(), serde_json::json!({}));

    // Persist to database
    {
        let agent = state.agent.read().await;
        if let Err(e) = agent.storage.insert_task(&task).await {
            return (StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: format!("Failed to save goal: {}", e) })).into_response();
        }
    }

    // Add to in-memory queue
    {
        let mut queue = state.tasks.write().await;
        queue.add(task);
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

/// Delete a goal
async fn delete_goal_endpoint(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    // Delete from database
    {
        let agent = state.agent.read().await;
        let _ = agent.storage.delete_task(&id).await;
    }

    // Remove from in-memory queue
    if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
        let mut queue = state.tasks.write().await;
        queue.remove(uuid);
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

/// Create a new task
async fn create_task(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> Response {
    use crate::core::{Task, TaskApproval, TaskStatus};

    // Convert and validate cron expression if provided
    // Standard 5-field cron is converted to 6-field (with seconds) for Rust cron crate
    let cron_expr = request.cron.as_ref().map(|expr| {
        if expr.split_whitespace().count() == 5 {
            format!("0 {}", expr) // Prepend "0 " for seconds
        } else {
            expr.clone()
        }
    });

    if let Some(ref cron) = cron_expr {
        if cron.parse::<cron::Schedule>().is_err() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Invalid cron expression: {}", cron),
                }),
            ).into_response();
        }
    }

    let approval = match request.approval.as_deref() {
        Some("require") => TaskApproval::RequireApproval,
        Some("notify") => TaskApproval::NotifyThenExecute { delay_seconds: 60 },
        _ => TaskApproval::Auto,
    };

    let status = if matches!(approval, TaskApproval::RequireApproval) {
        TaskStatus::AwaitingApproval
    } else {
        TaskStatus::Pending
    };

    let task = Task {
        id: uuid::Uuid::new_v4(),
        description: request.description,
        action: request.action.clone(),
        arguments: request.arguments,
        approval,
        capabilities: vec![request.action],
        status,
        created_at: chrono::Utc::now(),
        scheduled_for: None,
        cron: cron_expr,
        result: None,
        proof_id: None,
    };

    let is_scheduled = task.cron.is_some();
    // Access tasks directly without locking agent
    let mut tasks = state.tasks.write().await;
    let task_clone = task.clone();
    tasks.add(task);

    let save_result = {
        let agent = state.agent.read().await;
        agent.storage.insert_task(&task_clone).await
    };
    if let Err(e) = save_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to save task: {}", e),
            }),
        )
            .into_response();
    }

    let message = if is_scheduled {
        "Scheduled task created"
    } else {
        "Task created"
    };

    (StatusCode::OK, Json(serde_json::json!({"status": "ok", "message": message}))).into_response()
}

/// Update a task (description, arguments, cron)
async fn update_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateTaskRequest>,
) -> Response {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid task id".to_string(),
                }),
            )
                .into_response();
        }
    };

    let mut tasks = state.tasks.write().await;
    let Some(task) = tasks.get_mut(uuid) else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Task not found".to_string(),
            }),
        )
            .into_response();
    };

    let mut desc_to_save = None;
    let mut args_to_save = None;
    let mut cron_to_save = None;

    if let Some(description) = request.description {
        if !description.trim().is_empty() {
            task.description = description;
            desc_to_save = Some(task.description.clone());
        }
    }

    if let Some(arguments) = request.arguments {
        task.arguments = arguments;
        args_to_save = Some(serde_json::to_string(&task.arguments).unwrap_or_else(|_| "{}".to_string()));
    }

    if let Some(cron_value) = request.cron {
        let cron_clean = if cron_value.trim().is_empty() {
            None
        } else if cron_value.split_whitespace().count() == 5 {
            Some(format!("0 {}", cron_value))
        } else {
            Some(cron_value)
        };

        if let Some(ref cron) = cron_clean {
            if cron.parse::<cron::Schedule>().is_err() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("Invalid cron expression: {}", cron),
                    }),
                )
                    .into_response();
            }
        }

        task.cron = cron_clean;
        cron_to_save = task.cron.clone();
    }

    let save_result = {
        let agent = state.agent.read().await;
        agent.storage.update_task(&id, desc_to_save, args_to_save, cron_to_save, None).await
    };

    if let Err(e) = save_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to update task: {}", e),
            }),
        )
            .into_response();
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

/// Delete a task
async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid task id".to_string(),
                }),
            )
                .into_response();
        }
    };

    let mut tasks = state.tasks.write().await;
    let removed = tasks.remove(uuid);

    if removed {
        let delete_result = {
            let agent = state.agent.read().await;
            agent.storage.delete_task(&id).await
        };

        if let Err(e) = delete_result {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to delete task: {}", e),
                }),
            )
                .into_response();
        }

        (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Task not found".to_string(),
            }),
        )
            .into_response()
    }
}

/// Approve a task for execution
async fn approve_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid task id".to_string(),
                }),
            )
                .into_response();
        }
    };

    let mut tasks = state.tasks.write().await;
    let Some(task) = tasks.get_mut(uuid) else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Task not found".to_string(),
            }),
        )
            .into_response();
    };

    task.status = TaskStatus::Pending;
    let save_result = {
        let agent = state.agent.read().await;
        agent.storage.update_task_status(&id, &serde_json::to_string(&task.status).unwrap_or_else(|_| "Pending".to_string())).await
    };

    if let Err(e) = save_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to approve task: {}", e),
            }),
        )
            .into_response();
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

/// Reject a task
async fn reject_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid task id".to_string(),
                }),
            )
                .into_response();
        }
    };

    let mut tasks = state.tasks.write().await;
    let Some(task) = tasks.get_mut(uuid) else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Task not found".to_string(),
            }),
        )
            .into_response();
    };

    task.status = TaskStatus::Cancelled;
    let save_result = {
        let agent = state.agent.read().await;
        agent.storage.update_task_status(&id, &serde_json::to_string(&task.status).unwrap_or_else(|_| "Cancelled".to_string())).await
    };

    if let Err(e) = save_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to reject task: {}", e),
            }),
        )
            .into_response();
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

/// Plan a task using the LLM (returns a structured plan)
async fn plan_task(
    State(state): State<AppState>,
    Json(request): Json<PlanTaskRequest>,
) -> Response {
    const MAX_ACTIONS_FOR_PLAN: usize = 8;

    let (llm, actions) = {
        let agent = state.agent.read().await;
        let actions = match agent.runtime.list_actions().await {
            Ok(s) => s,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to list actions: {}", e),
                    }),
                )
                    .into_response();
            }
        };
        (agent.llm.clone(), actions)
    };

    let light_catalog = actions
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "description": s.description,
            })
        })
        .collect::<Vec<_>>();

    let selector_prompt = r#"You are a task planner for an AI agent.
Return ONLY valid JSON. Do not include any extra text.

Output schema:
{
  "summary": "short summary",
  "needed_actions": ["action_name", "action_name"]
}

Rules:
- Use only the provided actions.
- Keep the list minimal (only what is necessary).
"#;

    let mut selector_message = format!(
        "Task description: {}\n\nAvailable actions (names + descriptions):\n{}",
        request.description,
        serde_json::to_string_pretty(&light_catalog).unwrap_or_default()
    );
    if let Some(prompt) = request.prompt.as_ref() {
        if !prompt.trim().is_empty() {
            selector_message.push_str("\n\nRefinement request:\n");
            selector_message.push_str(prompt);
        }
    }

    let selector_response = match llm.chat(selector_prompt, &selector_message, &[], &actions).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("LLM planning failed: {}", e),
                }),
            )
                .into_response();
        }
    };

    let selector_json = extract_json(&selector_response.content).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Planner returned invalid JSON for action selection".to_string(),
            }),
        )
            .into_response()
    });

    let selector_json = match selector_json {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let needed_action_names: Vec<String> = selector_json
        .get("needed_actions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut needed_actions = actions
        .iter()
        .filter(|s| needed_action_names.iter().any(|n| n == &s.name))
        .cloned()
        .collect::<Vec<_>>();

    if needed_actions.is_empty() {
        needed_actions = actions.iter().take(MAX_ACTIONS_FOR_PLAN).cloned().collect();
    } else if needed_actions.len() > MAX_ACTIONS_FOR_PLAN {
        needed_actions.truncate(MAX_ACTIONS_FOR_PLAN);
    }

    let detailed_catalog = needed_actions
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "description": s.description,
                "input_schema": s.input_schema,
            })
        })
        .collect::<Vec<_>>();

    let plan_prompt = r#"You are a task planner for an AI agent.
Return ONLY valid JSON. Do not include any extra text.

Output schema:
{
  "summary": "short summary",
  "steps": [
    {
      "action": "action_name",
      "arguments": { "key": "value" },
      "rationale": "why this step is needed"
    }
  ],
  "notes": "optional"
}

Rules:
- Use only the provided actions.
- Provide JSON that is directly runnable.
- Keep steps minimal and ordered.
"#;

    let mut plan_message = format!(
        "Task description: {}\n\nAvailable actions (with schemas):\n{}",
        request.description,
        serde_json::to_string_pretty(&detailed_catalog).unwrap_or_default()
    );
    if let Some(prompt) = request.prompt.as_ref() {
        if !prompt.trim().is_empty() {
            plan_message.push_str("\n\nRefinement request:\n");
            plan_message.push_str(prompt);
        }
    }

    let plan_response = match llm.chat(plan_prompt, &plan_message, &[], &needed_actions).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("LLM planning failed: {}", e),
                }),
            )
                .into_response();
        }
    };

    let plan = extract_json(&plan_response.content);

    match plan {
        Some(plan) => (StatusCode::OK, Json(PlanTaskResponse { plan })).into_response(),
        None => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Planner returned invalid JSON".to_string(),
            }),
        )
            .into_response(),
    }
}

fn extract_json(text: &str) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .or_else(|| {
            let start = text.find('{')?;
            let end = text.rfind('}')?;
            serde_json::from_str::<serde_json::Value>(&text[start..=end]).ok()
        })
}

async fn gmail_oauth_start(State(state): State<AppState>) -> Response {
    // Try env var first, then fall back to secure config
    let config_dir = { state.agent.read().await.config_dir.clone() };
    let stored_creds = crate::core::config::SecureConfigManager::new(&config_dir)
        .ok()
        .and_then(|mgr| mgr.get_custom_secret("gmail_oauth_config").ok().flatten())
        .and_then(|json_str| serde_json::from_str::<serde_json::Value>(&json_str).ok());

    let client_id = std::env::var("GMAIL_CLIENT_ID").ok()
        .or_else(|| stored_creds.as_ref().and_then(|v| v.get("client_id").and_then(|c| c.as_str()).map(String::from)));

    let client_id = match client_id {
        Some(v) => v,
        None => return (StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Gmail not configured. Add credentials in Settings > Gmail.".to_string() })).into_response(),
    };

    let redirect_uri = "http://localhost:17990/oauth/callback";
    let scope = "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.send";

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&state=gmail&access_type=offline&prompt=consent",
        urlencoding::encode(&client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(scope),
    );

    (StatusCode::OK, Json(GmailOAuthStartResponse { auth_url })).into_response()
}

/// Exchange Gmail authorization code for tokens (called from oauth_callback)
async fn gmail_exchange_code(state: &AppState, code: &str) -> Result<(), String> {
    let config_dir = { state.agent.read().await.config_dir.clone() };
    let stored_creds = crate::core::config::SecureConfigManager::new(&config_dir)
        .ok()
        .and_then(|mgr| mgr.get_custom_secret("gmail_oauth_config").ok().flatten())
        .and_then(|json_str| serde_json::from_str::<serde_json::Value>(&json_str).ok());

    let client_id = std::env::var("GMAIL_CLIENT_ID").ok()
        .or_else(|| stored_creds.as_ref().and_then(|v| v.get("client_id").and_then(|c| c.as_str()).map(String::from)))
        .ok_or_else(|| "Gmail client_id not configured".to_string())?;
    let client_secret = std::env::var("GMAIL_CLIENT_SECRET").ok()
        .or_else(|| stored_creds.as_ref().and_then(|v| v.get("client_secret").and_then(|c| c.as_str()).map(String::from)))
        .ok_or_else(|| "Gmail client_secret not configured".to_string())?;

    let redirect_uri = "http://localhost:17990/oauth/callback";

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let params = [
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
    ];

    let resp = http_client.post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token exchange failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Token exchange failed ({}): {}", status, body));
    }

    #[derive(Deserialize)]
    struct TokenResp {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: i64,
    }

    let token: TokenResp = resp.json().await
        .map_err(|e| format!("Invalid token response: {}", e))?;

    let now = chrono::Utc::now().timestamp();
    let tokens = serde_json::json!({
        "access_token": token.access_token,
        "refresh_token": token.refresh_token.unwrap_or_default(),
        "expires_at": now + token.expires_in
    });

    // Store tokens encrypted via SecureConfigManager
    let manager = crate::core::config::SecureConfigManager::new(&config_dir)
        .map_err(|e| format!("Secure storage error: {}", e))?;
    let payload = serde_json::to_string(&tokens).unwrap_or_default();
    manager.set_custom_secret("gmail_tokens", Some(payload))
        .map_err(|e| format!("Failed to save tokens: {}", e))?;

    // Remove legacy plaintext token file if it exists
    let legacy_path = config_dir.join("gmail.json");
    if legacy_path.exists() {
        let _ = tokio::fs::remove_file(&legacy_path).await;
    }

    Ok(())
}

async fn gmail_status(State(state): State<AppState>) -> Response {
    let config_dir = {
        let agent = state.agent.read().await;
        agent.config_dir.clone()
    };
    let manager = match crate::core::config::SecureConfigManager::new(&config_dir) {
        Ok(m) => m,
        Err(_) => {
            return (StatusCode::OK, Json(serde_json::json!({"connected": false}))).into_response();
        }
    };
    let payload = match manager.get_custom_secret("gmail_tokens") {
        Ok(v) => v,
        Err(_) => None,
    };
    let payload = match payload {
        Some(v) => v,
        None => {
            return (StatusCode::OK, Json(serde_json::json!({"connected": false}))).into_response();
        }
    };
    let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap_or_else(|_| serde_json::json!({}));
    let expires_at = parsed.get("expires_at").and_then(|v| v.as_i64()).unwrap_or(0);
    let now = chrono::Utc::now().timestamp();
    let connected = expires_at > now;
    (StatusCode::OK, Json(serde_json::json!({"connected": connected, "expires_at": expires_at}))).into_response()
}

async fn gmail_test(State(state): State<AppState>) -> Response {
    let config_dir = {
        let agent = state.agent.read().await;
        agent.config_dir.clone()
    };
    let manager = match crate::core::config::SecureConfigManager::new(&config_dir) {
        Ok(m) => m,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Gmail not connected yet".to_string(),
                }),
            )
                .into_response();
        }
    };
    if manager.get_custom_secret("gmail_tokens").ok().flatten().is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Gmail not connected yet".to_string(),
            }),
        )
            .into_response();
    }

    let access_token = match crate::actions::gmail::ensure_access_token(&config_dir).await {
        Ok(token) => token,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Failed to refresh Gmail token: {}", e),
                }),
            )
                .into_response();
        }
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to build client: {}", e),
                }),
            )
                .into_response();
        }
    };

    let resp = client
        .get("https://gmail.googleapis.com/gmail/v1/users/me/profile")
        .bearer_auth(access_token)
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Gmail test failed: {}", e),
                }),
            )
                .into_response();
        }
    };

    if !resp.status().is_success() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Gmail test failed: {}", resp.status()),
            }),
        )
            .into_response();
    }

    #[derive(Deserialize)]
    struct GmailProfileResp {
        #[serde(default)]
        email_address: String,
    }

    let profile = resp.json::<GmailProfileResp>().await.unwrap_or(GmailProfileResp {
        email_address: "".to_string(),
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "connected": true,
            "email": profile.email_address
        })),
    )
        .into_response()
}

/// Get current settings
async fn get_settings(State(state): State<AppState>) -> Json<SettingsResponse> {
    let agent = state.agent.read().await;
    let config = &agent.config;
    let profile = state.user_profile.read().await;
    let daily_brief_channel = match agent.storage.get("daily_brief_channel").await {
        Ok(Some(bytes)) => String::from_utf8(bytes).unwrap_or_else(|_| "telegram".to_string()),
        _ => "telegram".to_string(),
    };

    // Primary LLM - has_key is true if api_key is set (including "[ENCRYPTED]" which means saved)
    let (provider, model, base_url, has_key) = match &config.llm {
        LlmProvider::Ollama { base_url, model } => {
            ("ollama".to_string(), model.clone(), Some(base_url.clone()), false)
        }
        LlmProvider::Anthropic { api_key, model } => {
            ("anthropic".to_string(), model.clone(), None, !api_key.is_empty())
        }
        LlmProvider::OpenAI { api_key, model, base_url } => {
            let provider = if base_url.is_some() { "openai-compatible" } else { "openai" };
            (provider.to_string(), model.clone(), base_url.clone(), !api_key.is_empty())
        }
    };

    // Fallback LLM
    let (fallback_provider, fallback_model, fallback_base_url, has_fallback_key) = match &config.llm_fallback {
        Some(LlmProvider::Ollama { base_url, model }) => {
            (Some("ollama".to_string()), Some(model.clone()), Some(base_url.clone()), false)
        }
        Some(LlmProvider::Anthropic { api_key, model }) => {
            (Some("anthropic".to_string()), Some(model.clone()), None, !api_key.is_empty())
        }
        Some(LlmProvider::OpenAI { api_key, model, base_url }) => {
            let provider = if base_url.is_some() { "openai-compatible" } else { "openai" };
            (Some(provider.to_string()), Some(model.clone()), base_url.clone(), !api_key.is_empty())
        }
        None => (None, None, None, false),
    };

    let (telegram_enabled, telegram_users, has_telegram_token) = match &config.telegram {
        Some(tg) => (true, tg.allowed_users.clone(), !tg.bot_token.is_empty()),
        None => (false, vec![], false),
    };

    // Settings are complete if name, model are set, and LLM is properly configured
    // Use has_key flag for consistency; also check base_url for openai-compatible
    let settings_complete = !config.name.trim().is_empty()
        && !model.trim().is_empty()
        && match &config.llm {
            LlmProvider::Ollama { base_url, .. } => !base_url.trim().is_empty(),
            LlmProvider::Anthropic { .. } => has_key,
            LlmProvider::OpenAI { base_url, .. } => {
                // For openai-compatible, also require base_url
                has_key && (base_url.is_none() || !base_url.as_ref().unwrap().trim().is_empty())
            }
        };

    Json(SettingsResponse {
        bot_name: config.name.clone(),
        personality: config.personality.clone(),
        timezone: profile.timezone.clone(),
        language: profile.language.clone(),
        tone: profile.tone.clone(),
        email_format: profile.email_format.clone(),
        daily_brief_channel,
        llm_provider: provider,
        llm_model: model,
        llm_base_url: base_url,
        has_api_key: has_key,
        llm_fallback_provider: fallback_provider,
        llm_fallback_model: fallback_model,
        llm_fallback_base_url: fallback_base_url,
        has_fallback_api_key: has_fallback_key,
        telegram_enabled,
        has_telegram_token,
        telegram_allowed_users: telegram_users,
        settings_complete,
    })
}

/// Get media generation settings (which providers are configured)
async fn get_media_settings(State(state): State<AppState>) -> Json<MediaSettingsResponse> {
    let agent = state.agent.read().await;

    // Check which media providers are configured (have API keys)
    let mut configured = Vec::new();
    for (provider, key) in &agent.config.media_gen.provider_api_keys {
        if !key.is_empty() && key != "[ENCRYPTED]" {
            configured.push(provider.clone());
        }
    }

    // Also check via integration (for runtime-configured providers)
    if let Some(media_gen) = agent.integrations.get("media_gen") {
        if let Ok(result) = media_gen.execute("list_providers", &serde_json::json!({})).await {
            if let Some(providers) = result.get("providers").and_then(|p| p.as_array()) {
                for p in providers {
                    if p.get("configured").and_then(|c| c.as_bool()).unwrap_or(false) {
                        if let Some(name) = p.get("provider").and_then(|n| n.as_str()) {
                            if !configured.contains(&name.to_string()) {
                                configured.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Get default/fallback providers from config
    let media_config = &agent.config.media_gen;

    Json(MediaSettingsResponse {
        configured,
        default_image_provider: media_config.default_image_provider.clone(),
        fallback_image_provider: media_config.fallback_image_provider.clone(),
        default_video_provider: media_config.default_video_provider.clone(),
        fallback_video_provider: media_config.fallback_video_provider.clone(),
    })
}

/// Update settings
async fn update_settings(
    State(state): State<AppState>,
    Json(settings): Json<SettingsUpdate>,
) -> Response {
    let result = {
        let mut agent_guard = state.agent.write().await;

        // Update bot name if provided
        if let Some(name) = &settings.bot_name {
            if !name.is_empty() {
                agent_guard.config.name = name.clone();
            }
        }

        // Update personality if provided
        if let Some(personality) = &settings.personality {
            if !personality.is_empty() {
                agent_guard.config.personality = personality.clone();
            }
        }

        if let Some(timezone) = settings.timezone.as_ref() {
            if !timezone.trim().is_empty() && timezone.parse::<chrono_tz::Tz>().is_err() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Invalid timezone. Use an IANA name like America/New_York".to_string(),
                    }),
                )
                    .into_response();
            }
        }

        if settings.timezone.is_some() || settings.language.is_some() || settings.tone.is_some() || settings.email_format.is_some() {
            let mut profile = agent_guard.user_profile.write().await;
            if let Some(timezone) = &settings.timezone {
                if timezone.trim().is_empty() {
                    profile.timezone = None;
                } else {
                    profile.timezone = Some(timezone.clone());
                }
            }
            if let Some(language) = &settings.language {
                profile.language = if language.trim().is_empty() { None } else { Some(language.clone()) };
            }
            if let Some(tone) = &settings.tone {
                profile.tone = if tone.trim().is_empty() { None } else { Some(tone.clone()) };
            }
            if let Some(email_format) = &settings.email_format {
                profile.email_format = if email_format.trim().is_empty() { None } else { Some(email_format.clone()) };
            }
            if let Ok(bytes) = serde_json::to_vec(&*profile) {
                if let Err(e) = agent_guard.storage.set("user_profile", &bytes).await {
                    tracing::warn!("Failed to persist user profile updates: {}", e);
                }
            }
        }

        if let Some(channel) = settings.daily_brief_channel.as_ref() {
            let normalized = channel.trim().to_lowercase();
            if normalized != "telegram" && normalized != "email" {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Daily brief channel must be 'telegram' or 'email'".to_string(),
                    }),
                )
                    .into_response();
            }

            if let Err(e) = agent_guard.storage.set("daily_brief_channel", normalized.as_bytes()).await {
                tracing::warn!("Failed to persist daily brief channel: {}", e);
            }

            let mut queue = agent_guard.tasks.write().await;
            let task_id = queue
                .all()
                .iter()
                .find(|t| t.action == "daily_brief")
                .map(|t| t.id);
            if let Some(id) = task_id {
                if let Some(task) = queue.get_mut(id) {
                    task.arguments = serde_json::json!({ "report_to": normalized });
                }
                let args = serde_json::to_string(&serde_json::json!({ "report_to": normalized }))
                    .unwrap_or_else(|_| "{}".to_string());
                let _ = agent_guard
                    .storage
                    .update_task(&id.to_string(), None, Some(args), None, None)
                    .await;
            }
        }

        // Get existing primary API key to preserve if not provided
        let existing_api_key = match &agent_guard.config.llm {
            LlmProvider::Anthropic { api_key, .. } => Some(api_key.clone()),
            LlmProvider::OpenAI { api_key, .. } => Some(api_key.clone()),
            _ => None,
        };

        // Get existing fallback API key to preserve if not provided
        let existing_fallback_api_key = agent_guard.config.llm_fallback.as_ref().and_then(|fb| {
            match fb {
                LlmProvider::Anthropic { api_key, .. } => Some(api_key.clone()),
                LlmProvider::OpenAI { api_key, .. } => Some(api_key.clone()),
                _ => None,
            }
        });

        let existing_telegram_token = agent_guard
            .config
            .telegram
            .as_ref()
            .map(|t| t.bot_token.clone());

        // Use new API key if provided, otherwise preserve existing (filter out "[ENCRYPTED]" placeholders)
        let api_key = settings.llm_api_key
            .filter(|k| !k.is_empty() && k != "[ENCRYPTED]")
            .or(existing_api_key.filter(|k| k != "[ENCRYPTED]"))
            .unwrap_or_default();

        // Fallback API key
        let fallback_api_key = settings.llm_fallback_api_key
            .filter(|k| !k.is_empty() && k != "[ENCRYPTED]")
            .or(existing_fallback_api_key.filter(|k| k != "[ENCRYPTED]"))
            .unwrap_or_default();

        // Handle empty base_url as None
        let base_url = settings.llm_base_url.filter(|u| !u.is_empty());

        // Basic validation
        if settings.llm_model.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "LLM model is required".to_string(),
                }),
            )
            .into_response();
        }

        if settings.llm_provider.as_str() != "ollama" && api_key.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "API key is required for the selected provider".to_string(),
                }),
            )
            .into_response();
        }

        if settings.llm_provider.as_str() == "ollama" {
            let url = base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string());
            if url.trim().is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Ollama base URL is required".to_string(),
                    }),
                )
                .into_response();
            }
        }
        if matches!(settings.llm_provider.as_str(), "openai-compatible" | "openrouter") {
            if base_url.as_deref().unwrap_or("").trim().is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Base URL is required for OpenAI-Compatible providers".to_string(),
                    }),
                )
                .into_response();
            }
        }

        // Build new LLM provider
        let new_llm = match settings.llm_provider.as_str() {
            "ollama" => LlmProvider::Ollama {
                base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
                model: settings.llm_model,
            },
            "anthropic" => LlmProvider::Anthropic {
                api_key: api_key.clone(),
                model: settings.llm_model,
            },
            "openai" => LlmProvider::OpenAI {
                api_key: api_key.clone(),
                model: settings.llm_model,
                base_url: None,
            },
            "openai-compatible" | "openrouter" => LlmProvider::OpenAI {
                api_key: api_key.clone(),
                model: settings.llm_model,
                base_url,
            },
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("Unknown provider: {}", settings.llm_provider),
                    }),
                ).into_response();
            }
        };

        // Build fallback LLM provider (optional)
        let fallback_base_url = settings.llm_fallback_base_url.filter(|u| !u.is_empty());
        let new_llm_fallback: Option<LlmProvider> = if let Some(fb_provider) = &settings.llm_fallback_provider {
            if !fb_provider.is_empty() && settings.llm_fallback_model.as_ref().map(|m| !m.is_empty()).unwrap_or(false) {
                let fb_model = settings.llm_fallback_model.clone().unwrap_or_default();
                match fb_provider.as_str() {
                    "ollama" => Some(LlmProvider::Ollama {
                        base_url: fallback_base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
                        model: fb_model,
                    }),
                    "anthropic" => Some(LlmProvider::Anthropic {
                        api_key: fallback_api_key.clone(),
                        model: fb_model,
                    }),
                    "openai" => Some(LlmProvider::OpenAI {
                        api_key: fallback_api_key.clone(),
                        model: fb_model,
                        base_url: None,
                    }),
                    "openai-compatible" | "openrouter" => Some(LlmProvider::OpenAI {
                        api_key: fallback_api_key.clone(),
                        model: fb_model,
                        base_url: fallback_base_url,
                    }),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        // Build telegram config
        let new_telegram = if settings.telegram_enabled {
            let token = settings
                .telegram_bot_token
                .filter(|t| !t.is_empty())
                .or(existing_telegram_token);

            if token.as_deref().unwrap_or("").trim().is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Telegram bot token is required when Telegram is enabled".to_string(),
                    }),
                )
                .into_response();
            }

            Some(TelegramConfig {
                bot_token: token.unwrap(),
                allowed_users: settings.telegram_allowed_users.unwrap_or_default(),
                dm_policy: "pairing".to_string(),
            })
        } else {
            None
        };

        // Test provider connection before saving
        if let Err(e) = test_llm_connection(&new_llm).await {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Provider test failed: {}", e),
                }),
            )
                .into_response();
        }

        // Update config
        agent_guard.config.llm = new_llm.clone();
        agent_guard.config.llm_fallback = new_llm_fallback;
        agent_guard.config.telegram = new_telegram;

        // Save media provider API keys to config (they will be encrypted by SecureConfigManager)
        for (provider, key) in &settings.media_providers {
            if !key.is_empty() && key != "[ENCRYPTED]" {
                agent_guard.config.media_gen.provider_api_keys.insert(provider.clone(), key.clone());
            }
        }

        // Update default/fallback media providers
        if let Some(ref provider) = settings.default_image_provider {
            agent_guard.config.media_gen.default_image_provider = Some(provider.clone());
        }
        if let Some(ref provider) = settings.fallback_image_provider {
            agent_guard.config.media_gen.fallback_image_provider = Some(provider.clone());
        }
        if let Some(ref provider) = settings.default_video_provider {
            agent_guard.config.media_gen.default_video_provider = Some(provider.clone());
        }
        if let Some(ref provider) = settings.fallback_video_provider {
            agent_guard.config.media_gen.fallback_video_provider = Some(provider.clone());
        }

        // Also configure runtime media_gen integration with the keys
        if !settings.media_providers.is_empty() {
            if let Some(media_gen) = agent_guard.integrations.get("media_gen") {
                for (provider, api_key) in &settings.media_providers {
                    if !api_key.is_empty() && api_key != "[ENCRYPTED]" {
                        let _ = media_gen.execute("configure_provider", &serde_json::json!({
                            "provider": provider,
                            "api_key": api_key
                        })).await;
                    }
                }
            }
        }

        // Save to disk
        let save_result = agent_guard.config.save(&agent_guard.config_dir);

        // Reinitialize LLM client
        match crate::core::LlmClient::new(&new_llm) {
            Ok(new_client) => {
                agent_guard.llm = new_client;
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to initialize LLM: {}", e),
                    }),
                ).into_response();
            }
        }

        save_result
    };

    match result {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "message": "Settings saved"})),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to save settings: {}", e),
            }),
        ).into_response(),
    }
}

async fn test_llm_connection(provider: &LlmProvider) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    match provider {
        LlmProvider::Ollama { base_url, .. } => {
            let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
            let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
            if resp.status().is_success() {
                Ok(())
            } else {
                Err(format!("Ollama returned {}", resp.status()))
            }
        }
        LlmProvider::OpenAI { api_key, base_url, .. } => {
            let base = base_url
                .as_deref()
                .unwrap_or("https://api.openai.com/v1")
                .trim_end_matches('/');
            let url = format!("{}/models", base);
            let resp = client
                .get(url)
                .bearer_auth(api_key)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if resp.status().is_success() {
                Ok(())
            } else {
                Err(format!("OpenAI-compatible returned {}", resp.status()))
            }
        }
        LlmProvider::Anthropic { api_key, .. } => {
            let url = "https://api.anthropic.com/v1/models";
            let resp = client
                .get(url)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if resp.status().is_success() {
                Ok(())
            } else {
                Err(format!("Anthropic returned {}", resp.status()))
            }
        }
    }
}

/// Get execution trace - Shows actual processing steps from last message + history
/// Uses independent Arcs - doesn't block during long operations
async fn get_trace(State(state): State<AppState>) -> Json<TraceResponse> {
    // Access trace data directly without locking agent
    let last_trace = state.last_trace.read().await;
    let trace_history = state.trace_history.read().await;

    // Convert agent's execution trace to API response format
    let trace: Vec<TraceStep> = last_trace.steps.iter().map(|step| {
        TraceStep {
            icon: step.icon.clone(),
            title: step.title.clone(),
            detail: step.detail.clone(),
            step_type: step.step_type.clone(),
            data: step.data.clone(),
            time: if let Some(ms) = step.duration_ms {
                format!("{} ({}ms)", step.timestamp.format("%H:%M:%S"), ms)
            } else {
                step.timestamp.format("%H:%M:%S").to_string()
            },
        }
    }).collect();

    // Build proof summary
    let proofs = if let Some(ref proof_id) = last_trace.proof_id {
        vec![ProofSummary {
            id: proof_id.clone(),
            message_preview: if last_trace.message.len() > 50 {
                format!("{}...", &last_trace.message[..50])
            } else {
                last_trace.message.clone()
            },
            time: last_trace.completed_at
                .map(|t| t.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| "pending".to_string()),
        }]
    } else {
        vec![]
    };

    // Build trace history summaries
    let history: Vec<TraceSummary> = trace_history.iter().map(|t| {
        let duration_ms = t.started_at.and_then(|start| {
            t.completed_at.map(|end| (end - start).num_milliseconds() as u64)
        });
        let status = if t.completed_at.is_some() { "completed" } else { "running" };
        TraceSummary {
            id: t.id.clone(),
            message_preview: if t.message.len() > 40 {
                format!("{}...", &t.message[..40])
            } else {
                t.message.clone()
            },
            channel: t.channel.clone(),
            status: status.to_string(),
            step_count: t.steps.len(),
            started_at: t.started_at.map(|s| s.format("%H:%M:%S").to_string()).unwrap_or_default(),
            duration_ms,
        }
    }).collect();

    Json(TraceResponse { trace, proofs, history })
}

/// Get details of a specific trace by ID
/// Uses independent Arc - doesn't block during long operations
async fn get_trace_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    // Access trace data directly without locking agent
    let trace_history = state.trace_history.read().await;

    // Find the trace by ID
    let trace = trace_history.iter().find(|t| t.id == id);

    match trace {
        Some(t) => {
            let duration_ms = t.started_at.and_then(|start| {
                t.completed_at.map(|end| (end - start).num_milliseconds() as u64)
            });

            let steps: Vec<TraceStep> = t.steps.iter().map(|step| {
                TraceStep {
                    icon: step.icon.clone(),
                    title: step.title.clone(),
                    detail: step.detail.clone(),
                    step_type: step.step_type.clone(),
                    data: step.data.clone(),
                    time: if let Some(ms) = step.duration_ms {
                        format!("{} ({}ms)", step.timestamp.format("%H:%M:%S"), ms)
                    } else {
                        step.timestamp.format("%H:%M:%S").to_string()
                    },
                }
            }).collect();

            (StatusCode::OK, Json(TraceDetailResponse {
                id: t.id.clone(),
                message: t.message.clone(),
                channel: t.channel.clone(),
                started_at: t.started_at.map(|s| s.format("%Y-%m-%d %H:%M:%S").to_string()),
                completed_at: t.completed_at.map(|c| c.format("%Y-%m-%d %H:%M:%S").to_string()),
                duration_ms,
                steps,
                response: t.response.clone(),
                proof_id: t.proof_id.clone(),
            })).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Trace '{}' not found", id),
            }),
        ).into_response(),
    }
}

/// Restart the server (Docker will auto-restart due to restart policy)
async fn restart_server() -> Response {
    tracing::info!("Restart requested via API - shutting down for restart");

    // Spawn a task to exit after a short delay (allows response to be sent)
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        std::process::exit(0);
    });

    (StatusCode::OK, Json(serde_json::json!({
        "status": "ok",
        "message": "Server is restarting..."
    }))).into_response()
}

// ============================================================================
// OAuth & Integrations
// ============================================================================

/// OAuth callback query parameters
#[derive(Debug, Deserialize)]
pub struct OAuthCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// Integration info for API response
#[derive(Debug, Serialize)]
pub struct IntegrationResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub status: String,
    pub auth_url: Option<String>,
}

/// Handle OAuth callback from providers (Google, etc.)
async fn oauth_callback(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<OAuthCallbackParams>,
) -> Response {
    // Check for OAuth error
    if let Some(error) = params.error {
        let html = format!(r#"<!DOCTYPE html>
<html>
<head><title>OAuth Failed</title>
<style>
body {{ font-family: system-ui; background: #1a1a2e; color: #eee; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; }}
.card {{ background: #16213e; padding: 2rem; border-radius: 12px; text-align: center; max-width: 400px; }}
.error {{ color: #ff6b6b; }}
</style>
</head>
<body>
<div class="card">
<h2 class="error">❌ Authorization Failed</h2>
<p>{}</p>
<p><a href="/" style="color: #00d9ff;">Return to CogniArk</a></p>
</div>
</body>
</html>"#, error);
        return (StatusCode::OK, Html(html)).into_response();
    }

    // Get code and state
    let code = match params.code {
        Some(c) => c,
        None => {
            return (StatusCode::BAD_REQUEST, Html("Missing authorization code")).into_response();
        }
    };

    let service_id = params.state.unwrap_or_else(|| "unknown".to_string());

    // Handle the callback based on service
    let result = match service_id.as_str() {
        "gmail" => {
            gmail_exchange_code(&state, &code).await
                .map(|_| serde_json::json!({"status": "connected"}))
                .map_err(|e| anyhow::anyhow!(e))
        }
        "google_calendar" => {
            let agent = state.agent.read().await;
            if let Some(calendar) = agent.integrations.get("google_calendar") {
                calendar.execute("auth_callback", &serde_json::json!({"code": code})).await
            } else {
                Err(anyhow::anyhow!("Google Calendar integration not configured"))
            }
        }
        "whatsapp" => {
            let agent = state.agent.read().await;
            if let Some(whatsapp) = agent.integrations.get("whatsapp") {
                whatsapp.execute("auth_callback", &serde_json::json!({"code": code})).await
            } else {
                Err(anyhow::anyhow!("WhatsApp integration not configured"))
            }
        }
        _ => Err(anyhow::anyhow!("Unknown service: {}", service_id)),
    };

    match result {
        Ok(_) => {
            let html = format!(r#"<!DOCTYPE html>
<html>
<head><title>Connected!</title>
<style>
body {{ font-family: system-ui; background: #1a1a2e; color: #eee; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; }}
.card {{ background: #16213e; padding: 2rem; border-radius: 12px; text-align: center; max-width: 400px; }}
.success {{ color: #00d9ff; }}
</style>
</head>
<body>
<div class="card">
<h2 class="success">✅ {} Connected!</h2>
<p>You can close this window and return to CogniArk.</p>
<p><a href="/" style="color: #00d9ff;">Return to CogniArk</a></p>
</div>
<script>setTimeout(() => window.close(), 3000);</script>
</body>
</html>"#, service_id.replace('_', " "));
            (StatusCode::OK, Html(html)).into_response()
        }
        Err(e) => {
            let html = format!(r#"<!DOCTYPE html>
<html>
<head><title>Connection Failed</title>
<style>
body {{ font-family: system-ui; background: #1a1a2e; color: #eee; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; }}
.card {{ background: #16213e; padding: 2rem; border-radius: 12px; text-align: center; max-width: 400px; }}
.error {{ color: #ff6b6b; }}
</style>
</head>
<body>
<div class="card">
<h2 class="error">❌ Connection Failed</h2>
<p>{}</p>
<p><a href="/" style="color: #00d9ff;">Return to CogniArk</a></p>
</div>
</body>
</html>"#, e);
            (StatusCode::OK, Html(html)).into_response()
        }
    }
}

/// List all integrations with their status
async fn list_integrations(State(state): State<AppState>) -> Response {
    let agent = state.agent.read().await;

    let mut integrations = Vec::new();
    for info in agent.integrations.list().await {
        let status_str = match info.status {
            crate::integrations::IntegrationStatus::NotConfigured => "not_configured",
            crate::integrations::IntegrationStatus::NeedsAuth => "needs_auth",
            crate::integrations::IntegrationStatus::Connected => "connected",
            crate::integrations::IntegrationStatus::Error(_) => "error",
        };

        // Get auth URL if needs auth
        let auth_url = if matches!(info.status, crate::integrations::IntegrationStatus::NeedsAuth) {
            if let Some(integration) = agent.integrations.get(&info.id) {
                integration.execute("get_auth_url", &serde_json::json!({"state": info.id}))
                    .await
                    .ok()
                    .and_then(|v| v.get("url").and_then(|u| u.as_str()).map(String::from))
            } else {
                None
            }
        } else {
            None
        };

        integrations.push(IntegrationResponse {
            id: info.id,
            name: info.name,
            description: info.description,
            icon: info.icon,
            status: status_str.to_string(),
            auth_url,
        });
    }

    (StatusCode::OK, Json(serde_json::json!({ "integrations": integrations }))).into_response()
}

/// Get auth URL for a specific integration
async fn get_integration_auth_url(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let agent = state.agent.read().await;

    match agent.integrations.get(&id) {
        Some(integration) => {
            match integration.execute("get_auth_url", &serde_json::json!({"state": id})).await {
                Ok(result) => (StatusCode::OK, Json(result)).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse { error: e.to_string() }),
                ).into_response(),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: format!("Integration '{}' not found", id) }),
        ).into_response(),
    }
}

/// Disconnect an integration
async fn disconnect_integration(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let agent = state.agent.read().await;

    match agent.integrations.get(&id) {
        Some(integration) => {
            match integration.execute("disconnect", &serde_json::json!({})).await {
                Ok(_) => (
                    StatusCode::OK,
                    Json(serde_json::json!({"status": "ok", "message": "Disconnected"})),
                ).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse { error: e.to_string() }),
                ).into_response(),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: format!("Integration '{}' not found", id) }),
        ).into_response(),
    }
}

/// Configure an integration (store OAuth credentials)
async fn configure_integration(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<serde_json::Value>,
) -> Response {
    let client_id = match request.get("client_id").and_then(|v| v.as_str()) {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => return (StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Missing client_id".to_string() })).into_response(),
    };
    let client_secret = match request.get("client_secret").and_then(|v| v.as_str()) {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => return (StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Missing client_secret".to_string() })).into_response(),
    };

    let config_dir = {
        let agent = state.agent.read().await;
        agent.config_dir.clone()
    };
    let manager = match crate::core::config::SecureConfigManager::new(&config_dir) {
        Ok(m) => m,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: format!("Config error: {}", e) })).into_response(),
    };

    let creds = serde_json::json!({
        "client_id": client_id,
        "client_secret": client_secret,
        "redirect_uri": "http://localhost:17990/oauth/callback"
    });

    let key = format!("{}_oauth_config", id);
    match manager.set_custom_secret(&key, Some(creds.to_string())) {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "configured"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: format!("Failed to save: {}", e) })).into_response(),
    }
}

/// Configure Gmail OAuth credentials
async fn configure_gmail(
    State(state): State<AppState>,
    Json(request): Json<serde_json::Value>,
) -> Response {
    let client_id = match request.get("client_id").and_then(|v| v.as_str()) {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => return (StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Missing client_id".to_string() })).into_response(),
    };
    let client_secret = match request.get("client_secret").and_then(|v| v.as_str()) {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => return (StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Missing client_secret".to_string() })).into_response(),
    };

    let config_dir = {
        let agent = state.agent.read().await;
        agent.config_dir.clone()
    };
    let manager = match crate::core::config::SecureConfigManager::new(&config_dir) {
        Ok(m) => m,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: format!("Config error: {}", e) })).into_response(),
    };

    let creds = serde_json::json!({
        "client_id": client_id,
        "client_secret": client_secret
    });

    match manager.set_custom_secret("gmail_oauth_config", Some(creds.to_string())) {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: format!("Failed to save: {}", e) })).into_response(),
    }
}
