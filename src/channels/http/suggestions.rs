use super::*;

use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(super) struct ChatSuggestionOutcome {
    pub kind: String,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub view: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct SuggestionRunSnapshot {
    task_ids: HashMap<String, crate::storage::entities::task::Model>,
    watcher_ids: HashMap<String, crate::core::watcher::Watcher>,
    app_ids: HashMap<String, serde_json::Value>,
}

fn normalize_task_status(raw: &str) -> String {
    let parsed = serde_json::from_str::<String>(raw).unwrap_or_else(|_| raw.to_string());
    let trimmed = parsed.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    let mut out = String::new();
    for (idx, ch) in trimmed.chars().enumerate() {
        if ch.is_uppercase() && idx > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn watcher_status_label(status: &crate::core::watcher::WatcherStatus) -> String {
    match status {
        crate::core::watcher::WatcherStatus::Active => "active".to_string(),
        crate::core::watcher::WatcherStatus::Paused => "paused".to_string(),
        crate::core::watcher::WatcherStatus::Triggered => "triggered".to_string(),
        crate::core::watcher::WatcherStatus::TimedOut => "timed_out".to_string(),
        crate::core::watcher::WatcherStatus::Cancelled => "cancelled".to_string(),
        crate::core::watcher::WatcherStatus::Failed { .. } => "failed".to_string(),
    }
}

fn outcome_kind_priority(suggestion_kind: &str, outcome_kind: &str) -> usize {
    match suggestion_kind {
        "app" => match outcome_kind {
            "app" => 0,
            "watcher" => 1,
            "task" => 2,
            _ => 3,
        },
        "watcher" => match outcome_kind {
            "watcher" => 0,
            "task" => 1,
            "app" => 2,
            _ => 3,
        },
        "task" => match outcome_kind {
            "task" => 0,
            "watcher" => 1,
            "app" => 2,
            _ => 3,
        },
        "workflow" => match outcome_kind {
            "watcher" => 0,
            "task" => 1,
            "app" => 2,
            _ => 3,
        },
        _ => 3,
    }
}

pub(super) async fn capture_run_snapshot(state: &AppState) -> SuggestionRunSnapshot {
    let (storage, watcher_rows) = {
        let agent = state.agent.read().await;
        (agent.storage.clone(), agent.watcher_manager.list().await)
    };

    let task_rows = storage.get_tasks().await.unwrap_or_default();
    let app_rows = state.app_registry.list().await;

    SuggestionRunSnapshot {
        task_ids: task_rows
            .into_iter()
            .map(|task| (task.id.clone(), task))
            .collect(),
        watcher_ids: watcher_rows
            .into_iter()
            .map(|watcher| (watcher.id.to_string(), watcher))
            .collect(),
        app_ids: app_rows
            .into_iter()
            .filter_map(|app| {
                let rec = app.as_object()?;
                let id = rec.get("id")?.as_str()?.trim().to_string();
                if id.is_empty() {
                    None
                } else {
                    Some((id, serde_json::Value::Object(rec.clone())))
                }
            })
            .collect(),
    }
}

pub(super) async fn collect_run_outcomes(
    state: &AppState,
    before: &SuggestionRunSnapshot,
    suggestion_kind: &str,
) -> Vec<ChatSuggestionOutcome> {
    let after = capture_run_snapshot(state).await;
    let mut outcomes = Vec::new();

    for (id, task) in after.task_ids {
        if before.task_ids.contains_key(&id) {
            continue;
        }
        let title = if task.description.trim().is_empty() {
            "Created task".to_string()
        } else {
            task.description.trim().to_string()
        };
        let detail = format!("Action: {}", task.action);
        let status = normalize_task_status(&task.status);
        let created_at = task.created_at;
        outcomes.push(ChatSuggestionOutcome {
            kind: "task".to_string(),
            id,
            title,
            detail: Some(detail),
            status: Some(status),
            url: None,
            view: Some("tasks".to_string()),
            created_at: Some(created_at),
            primary: false,
        });
    }

    for (id, watcher) in after.watcher_ids {
        if before.watcher_ids.contains_key(&id) {
            continue;
        }
        let title = if watcher.description.trim().is_empty() {
            "Created watcher".to_string()
        } else {
            watcher.description.trim().to_string()
        };
        let detail = format!(
            "Polls {} every {}s",
            watcher.poll_action, watcher.interval_secs
        );
        let status = watcher_status_label(&watcher.status);
        let created_at = watcher.created_at.to_rfc3339();
        outcomes.push(ChatSuggestionOutcome {
            kind: "watcher".to_string(),
            id,
            title,
            detail: Some(detail),
            status: Some(status),
            url: None,
            view: Some("watchers".to_string()),
            created_at: Some(created_at),
            primary: false,
        });
    }

    for (id, app) in after.app_ids {
        if before.app_ids.contains_key(&id) {
            continue;
        }
        let title = app
            .get("title")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Deployed app")
            .to_string();
        let runtime_mode = app
            .get("runtime_mode")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        outcomes.push(ChatSuggestionOutcome {
            kind: "app".to_string(),
            id,
            title,
            detail: (!runtime_mode.is_empty()).then(|| format!("Runtime: {}", runtime_mode)),
            status: Some(
                app.get("running")
                    .and_then(|value| value.as_bool())
                    .map(|running| if running { "running" } else { "stopped" })
                    .unwrap_or("unknown")
                    .to_string(),
            ),
            url: app
                .get("access_url")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
            view: Some("apps".to_string()),
            created_at: app
                .get("created_at")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
            primary: false,
        });
    }

    outcomes.sort_by(|a, b| {
        outcome_kind_priority(suggestion_kind, &a.kind)
            .cmp(&outcome_kind_priority(suggestion_kind, &b.kind))
            .then_with(|| a.title.cmp(&b.title))
    });
    if let Some(first) = outcomes.first_mut() {
        first.primary = true;
    }
    outcomes
}

pub(super) async fn update_chat_suggestion_after_run(
    storage: &crate::storage::Storage,
    suggestion_id: &str,
    trace_id: &str,
    run_status: &str,
    completed_at: &str,
    last_run_error: Option<String>,
    accepted_outcomes: Vec<ChatSuggestionOutcome>,
) {
    let mut suggestions = load_chat_suggestions(storage).await;
    let Some(idx) = suggestions.iter().position(|item| item.id == suggestion_id) else {
        return;
    };

    suggestions[idx].run_status = Some(run_status.to_string());
    suggestions[idx].last_run_completed_at = Some(completed_at.to_string());
    suggestions[idx].last_run_error = last_run_error;
    suggestions[idx].updated_at = completed_at.to_string();
    suggestions[idx].accepted_goal_id = None;
    suggestions[idx].accepted_outcomes = accepted_outcomes;
    if !trace_id.trim().is_empty() {
        suggestions[idx].accepted_trace_id = Some(trace_id.to_string());
    }
    if run_status == "failed" {
        suggestions[idx].status = "open".to_string();
        suggestions[idx].accepted_at = None;
    } else {
        suggestions[idx].status = "accepted".to_string();
    }
    suggestions = prune_chat_suggestion_history(suggestions);
    save_chat_suggestions(storage, &suggestions).await;
}

pub(super) async fn get_autonomy_suggestion_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let storage = { state.agent.read().await.storage.clone() };
    let suggestions = load_chat_suggestions(&storage).await;
    match suggestions
        .into_iter()
        .find(|suggestion| suggestion.id == id)
    {
        Some(suggestion) => (
            StatusCode::OK,
            Json(serde_json::json!({ "suggestion": suggestion })),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Suggestion not found".to_string(),
            }),
        )
            .into_response(),
    }
}
