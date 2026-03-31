use anyhow::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::core::{ExecutionRun, ExecutionRunStatus, ToolAttempt};
use crate::storage::{
    experience_edge, experience_item, experience_run, learning_candidate, procedural_pattern,
    Storage,
};

pub const LEARNING_ENABLED_KEY: &str = "learning_enabled_v1";
pub const LEARNING_LOCAL_ONLY_KEY: &str = "learning_local_only_v1";
pub const LEARNING_MODEL_SLOT_KEY: &str = "learning_model_slot_v1";
pub const LEARNING_QUEUE_CAP_KEY: &str = "learning_queue_cap_v1";
const CORRECTION_WINDOW_MINUTES: i64 = 30;
const DEFAULT_QUEUE_CAP: usize = 64;

fn safe_truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect::<String>()
}

fn stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    for part in parts {
        hasher.update([0u8]);
        hasher.update(part.as_bytes());
    }
    let digest = hex::encode(hasher.finalize());
    format!("{}-{}", prefix, &digest[..24])
}

fn short_hash(parts: &[&str]) -> String {
    stable_id("h", parts)
        .rsplit('-')
        .next()
        .unwrap_or("candidate")
        .to_string()
}

fn scope_from_ids(project_id: Option<&str>, conversation_id: Option<&str>) -> &'static str {
    if conversation_id.is_some() {
        "conversation"
    } else if project_id.is_some() {
        "project"
    } else {
        "global"
    }
}

fn normalize_token(token: &str) -> Option<String> {
    let trimmed = token
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric())
        .to_ascii_lowercase();
    if trimmed.len() < 3 {
        return None;
    }
    if matches!(
        trimmed.as_str(),
        "that"
            | "this"
            | "with"
            | "from"
            | "have"
            | "need"
            | "please"
            | "your"
            | "about"
            | "after"
            | "before"
            | "there"
            | "their"
            | "would"
            | "should"
    ) {
        return None;
    }
    Some(trimmed)
}

fn derive_intent_key(message: &str, task_type: &str) -> String {
    let mut tokens = message
        .split_whitespace()
        .filter_map(normalize_token)
        .take(6)
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        tokens.push("general".to_string());
    }
    format!("{}::{}", task_type, tokens.join("-"))
}

fn tool_sequence_digest(tool_attempts: &[ToolAttempt]) -> Option<String> {
    if tool_attempts.is_empty() {
        return None;
    }
    let sequence = tool_attempts
        .iter()
        .map(|attempt| attempt.tool_name.as_str())
        .collect::<Vec<_>>();
    Some(short_hash(&sequence))
}

fn tool_sequence_json(tool_attempts: &[ToolAttempt]) -> Value {
    Value::Array(
        tool_attempts
            .iter()
            .map(|attempt| {
                json!({
                    "tool_name": attempt.tool_name,
                    "status": attempt.status.as_str(),
                    "sequence_no": attempt.sequence_no,
                    "retryable": attempt.retryable,
                    "side_effect_level": attempt.side_effect_level,
                })
            })
            .collect(),
    )
}

fn tool_names_from_value(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("tool_name").and_then(|value| value.as_str()))
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn suggested_steps_from_tools(tool_names: &[String]) -> Vec<String> {
    if tool_names.is_empty() {
        return vec!["Review the latest run context and complete the task directly.".to_string()];
    }
    tool_names
        .iter()
        .enumerate()
        .map(|(index, tool)| {
            if index == tool_names.len().saturating_sub(1) {
                format!("Finish with `{}` and return the concrete result.", tool)
            } else {
                format!(
                    "Use `{}` as step {} in the learned sequence.",
                    tool,
                    index + 1
                )
            }
        })
        .collect()
}

fn extract_operating_constraint(message: &str) -> Option<String> {
    let lowered = message.trim().to_ascii_lowercase();
    if lowered.is_empty() {
        return None;
    }
    if lowered.contains("always ") || lowered.starts_with("always ") {
        return Some(safe_truncate(message.trim(), 220));
    }
    if lowered.contains("never ") || lowered.starts_with("never ") {
        return Some(safe_truncate(message.trim(), 220));
    }
    if lowered.contains("don't ") || lowered.contains("do not ") {
        return Some(safe_truncate(message.trim(), 220));
    }
    None
}

fn bool_like(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

fn humanize_fact_key(key: &str) -> String {
    key.split('_')
        .filter(|part| !part.trim().is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn describe_user_preference_memory(
    key: &str,
    value: &str,
) -> Option<(&'static str, String, String)> {
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return None;
    }

    let normalized_value = safe_truncate(value, 220);
    let (kind, title, content) = match key {
        "user_name" => (
            "personal_fact",
            "Learned personal fact".to_string(),
            format!("The user's preferred name is {}.", normalized_value),
        ),
        "rule_require_explicit_approval_before_side_effects" => (
            "constraint",
            "Learned operating constraint".to_string(),
            if bool_like(value).unwrap_or(true) {
                "Require explicit approval before side-effecting actions.".to_string()
            } else {
                "Explicit approval before side-effecting actions is not required.".to_string()
            },
        ),
        "rule_show_plan_before_side_effects" => (
            "constraint",
            "Learned operating constraint".to_string(),
            if bool_like(value).unwrap_or(true) {
                "Show the plan before side-effecting actions.".to_string()
            } else {
                "Showing the plan before side-effecting actions is optional.".to_string()
            },
        ),
        _ if key.starts_with("likes_") => (
            "personal_fact",
            "Learned personal fact".to_string(),
            format!("The user likes {}.", normalized_value),
        ),
        _ if key.starts_with("dislikes_") => (
            "personal_fact",
            "Learned personal fact".to_string(),
            format!("The user dislikes {}.", normalized_value),
        ),
        _ if key.starts_with("rule_") => (
            "constraint",
            "Learned operating constraint".to_string(),
            format!("{}: {}.", humanize_fact_key(key), normalized_value),
        ),
        _ => (
            "personal_fact",
            "Learned personal fact".to_string(),
            format!("{}: {}.", humanize_fact_key(key), normalized_value),
        ),
    };

    Some((kind, title, content))
}

fn candidate_action_name(pattern: &procedural_pattern::Model) -> String {
    let mut slug = pattern
        .title
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug = slug.trim_matches('-').to_string();
    let base = if slug.is_empty() {
        "learned-workflow".to_string()
    } else {
        format!("learned-{}", safe_truncate(&slug, 28))
    };
    format!(
        "{}-{}",
        base.trim_matches('-'),
        short_hash(&[pattern.id.as_str()])
    )
}

fn workflow_candidate_markdown(
    pattern: &procedural_pattern::Model,
    action_name: &str,
    steps: &[String],
) -> String {
    let description = pattern.summary.replace('"', "'").replace(['\n', '\r'], " ");
    let workflow_steps = if steps.is_empty() {
        "- Review the request context and follow the learned sequence.".to_string()
    } else {
        steps
            .iter()
            .enumerate()
            .map(|(index, step)| format!("### Step {}\n{}\n", index + 1, step))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "---\nname: {action_name}\ndescription: \"{description}\"\nversion: \"1.0.0\"\npermissions: [memory, research]\n---\n\n# {title}\n\n{summary}\n\n## Trigger\n{trigger}\n\n## Workflow\n\n{workflow_steps}\n",
        title = pattern.title,
        summary = pattern.summary,
        trigger = if pattern.trigger_summary.trim().is_empty() {
            "Use this workflow when the request matches the learned pattern."
        } else {
            pattern.trigger_summary.as_str()
        },
    )
}

pub async fn load_learning_enabled(storage: &Storage) -> bool {
    storage
        .get(LEARNING_ENABLED_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|value| !value.trim().eq_ignore_ascii_case("false"))
        .unwrap_or(true)
}

pub async fn load_learning_local_only(storage: &Storage) -> bool {
    storage
        .get(LEARNING_LOCAL_ONLY_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|value| !value.trim().eq_ignore_ascii_case("false"))
        .unwrap_or(true)
}

pub async fn load_learning_model_slot(storage: &Storage) -> Option<String> {
    storage
        .get(LEARNING_MODEL_SLOT_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub async fn load_learning_queue_cap(storage: &Storage) -> usize {
    storage
        .get(LEARNING_QUEUE_CAP_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_QUEUE_CAP)
}

fn build_experience_run_id(execution_run_id: &str) -> String {
    stable_id("exprun", &[execution_run_id])
}

fn build_item_id(
    kind: &str,
    scope: &str,
    project_id: Option<&str>,
    conversation_id: Option<&str>,
    normalized_key: &str,
) -> String {
    stable_id(
        "expitem",
        &[
            kind,
            scope,
            project_id.unwrap_or(""),
            conversation_id.unwrap_or(""),
            normalized_key,
        ],
    )
}

fn build_pattern_id(
    scope: &str,
    project_id: Option<&str>,
    conversation_id: Option<&str>,
    intent_key: &str,
    tool_sequence_digest: Option<&str>,
) -> String {
    stable_id(
        "pattern",
        &[
            scope,
            project_id.unwrap_or(""),
            conversation_id.unwrap_or(""),
            intent_key,
            tool_sequence_digest.unwrap_or(""),
        ],
    )
}

pub async fn record_execution_experience(
    storage: &Storage,
    execution_run: &ExecutionRun,
    message: &str,
    channel: &str,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    prompt_version: Option<&str>,
    strategy_version: Option<&str>,
    policy_version: Option<&str>,
    model_slot: Option<&str>,
) -> Result<()> {
    if !load_learning_enabled(storage).await {
        return Ok(());
    }
    let task_type = crate::core::self_evolve::strategy_runtime::infer_task_type(message);
    let intent_key = derive_intent_key(message, &task_type);
    let scope = scope_from_ids(project_id, conversation_id).to_string();
    let tool_attempts = storage
        .list_tool_attempts_for_run(&execution_run.id)
        .await
        .unwrap_or_default();
    let sequence_json = tool_sequence_json(&tool_attempts);
    let sequence_digest = tool_sequence_digest(&tool_attempts);
    let success_state = if matches!(
        execution_run.status,
        ExecutionRunStatus::Completed | ExecutionRunStatus::Degraded
    ) {
        "provisional"
    } else {
        "failed"
    };
    let metadata = json!({
        "execution_status": execution_run.status.as_str(),
        "degradation": execution_run.degradation,
        "attempted_models": execution_run.attempted_models,
        "last_error": execution_run.last_error,
        "tool_count": tool_attempts.len(),
        "degraded": matches!(execution_run.status, ExecutionRunStatus::Degraded),
    });
    let experience_id = build_experience_run_id(&execution_run.id);
    let now = chrono::Utc::now().to_rfc3339();
    storage
        .upsert_experience_run(&experience_run::Model {
            id: experience_id.clone(),
            execution_run_id: Some(execution_run.id.clone()),
            trace_id: execution_run.trace_id.clone(),
            conversation_id: conversation_id.map(|value| value.to_string()),
            project_id: project_id.map(|value| value.to_string()),
            channel: channel.to_string(),
            scope,
            intent_key,
            task_type: Some(task_type),
            request_text: Some(safe_truncate(message.trim(), 2000)),
            tool_sequence_digest: sequence_digest.clone(),
            tool_sequence_json: sequence_json,
            strategy_version: strategy_version.map(|value| value.to_string()),
            policy_version: policy_version.map(|value| value.to_string()),
            prompt_version: prompt_version.map(|value| value.to_string()),
            model_slot: model_slot.map(|value| value.to_string()),
            success_state: success_state.to_string(),
            correction_state: "none".to_string(),
            outcome_summary: execution_run.result_summary.clone(),
            failure_reason: execution_run.last_error.clone(),
            metadata,
            consolidated: false,
            accepted_at: None,
            corrected_at: None,
            created_at: execution_run.created_at.clone(),
            updated_at: execution_run.updated_at.clone(),
        })
        .await?;

    for attempt in tool_attempts {
        let edge_type = if attempt.status.as_str() == "success" {
            "succeeded_with"
        } else {
            "failed_with"
        };
        storage
            .upsert_experience_edge(&experience_edge::Model {
                id: stable_id(
                    "edge",
                    &[
                        experience_id.as_str(),
                        edge_type,
                        attempt.tool_name.as_str(),
                        &attempt.sequence_no.to_string(),
                    ],
                ),
                source_ref: experience_id.clone(),
                source_kind: "experience_run".to_string(),
                target_ref: format!("tool:{}", attempt.tool_name),
                target_kind: "tool".to_string(),
                edge_type: edge_type.to_string(),
                weight: if edge_type == "succeeded_with" {
                    1.0
                } else {
                    0.35
                },
                source_run_id: Some(experience_id.clone()),
                metadata: json!({
                    "tool_name": attempt.tool_name,
                    "tool_status": attempt.status.as_str(),
                    "sequence_no": attempt.sequence_no,
                }),
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await?;
    }

    Ok(())
}

pub async fn record_user_correction(
    storage: &Storage,
    conversation_id: &str,
    message: &str,
) -> Result<()> {
    if !load_learning_enabled(storage).await {
        return Ok(());
    }
    let _ = storage
        .mark_latest_provisional_experience_run_corrected(
            conversation_id,
            &safe_truncate(message.trim(), 180),
            CORRECTION_WINDOW_MINUTES,
        )
        .await?;
    Ok(())
}

fn positive_procedure_summary(run: &experience_run::Model, tool_names: &[String]) -> String {
    if tool_names.is_empty() {
        format!(
            "For `{}`, the successful pattern was to solve the task directly and return the result.",
            run.intent_key
        )
    } else {
        format!(
            "For `{}`, the successful pattern was: {}.",
            run.intent_key,
            tool_names.join(" -> ")
        )
    }
}

fn negative_lesson_summary(run: &experience_run::Model, tool_names: &[String]) -> String {
    let sequence = if tool_names.is_empty() {
        "the recent approach".to_string()
    } else {
        tool_names.join(" -> ")
    };
    format!(
        "Avoid repeating `{}` with {} when the user is correcting or the run failed.",
        run.intent_key, sequence
    )
}

async fn upsert_constraint_from_message(
    storage: &Storage,
    message: &str,
    project_id: Option<&str>,
    conversation_id: Option<&str>,
) -> Result<()> {
    let Some(content) = extract_operating_constraint(message) else {
        return Ok(());
    };
    let scope = scope_from_ids(project_id, conversation_id);
    let normalized_key = format!(
        "constraint::{}",
        short_hash(&[
            scope,
            project_id.unwrap_or(""),
            conversation_id.unwrap_or(""),
            content.as_str()
        ])
    );
    let id = build_item_id(
        "constraint",
        scope,
        project_id,
        conversation_id,
        &normalized_key,
    );
    let existing = storage.get_experience_item(&id).await?;
    let support_count = existing
        .as_ref()
        .map(|item| item.support_count.saturating_add(1))
        .unwrap_or(1);
    let confidence = existing
        .as_ref()
        .map(|item| (item.confidence + 0.06).min(0.98))
        .unwrap_or(0.72);
    let now = chrono::Utc::now().to_rfc3339();
    storage
        .upsert_experience_item(&experience_item::Model {
            id,
            kind: "constraint".to_string(),
            scope: scope.to_string(),
            project_id: project_id.map(|value| value.to_string()),
            conversation_id: conversation_id.map(|value| value.to_string()),
            title: "Learned operating constraint".to_string(),
            content,
            normalized_key,
            confidence,
            support_count,
            contradiction_count: existing
                .as_ref()
                .map(|item| item.contradiction_count)
                .unwrap_or_default(),
            status: "active".to_string(),
            metadata: json!({
                "source": "experience_consolidation",
                "constraint_type": "user_instruction",
            }),
            last_supported_at: Some(now.clone()),
            last_contradicted_at: existing.and_then(|item| item.last_contradicted_at),
            created_at: now.clone(),
            updated_at: now,
        })
        .await?;
    Ok(())
}

pub async fn sync_user_preference_to_experience_item(
    storage: &Storage,
    key: &str,
    value: &str,
    confidence: f64,
    source: &str,
) -> Result<()> {
    let Some((kind, title, content)) = describe_user_preference_memory(key, value) else {
        return Ok(());
    };
    let normalized_key = format!("user_pref::{}", key.trim());
    let id = build_item_id(kind, "global", None, None, &normalized_key);
    let existing = storage.get_experience_item(&id).await?;
    let now = chrono::Utc::now().to_rfc3339();
    let support_count = existing
        .as_ref()
        .map(|item| item.support_count.saturating_add(1))
        .unwrap_or(1);
    let merged_confidence = existing
        .as_ref()
        .map(|item| item.confidence.max(confidence as f64).min(0.99))
        .unwrap_or(confidence as f64);

    storage
        .upsert_experience_item(&experience_item::Model {
            id,
            kind: kind.to_string(),
            scope: "global".to_string(),
            project_id: None,
            conversation_id: None,
            title,
            content,
            normalized_key,
            confidence: merged_confidence,
            support_count,
            contradiction_count: existing
                .as_ref()
                .map(|item| item.contradiction_count)
                .unwrap_or_default(),
            status: "active".to_string(),
            metadata: json!({
                "source": source,
                "user_preference_key": key.trim(),
                "user_preference_value": safe_truncate(value.trim(), 220),
            }),
            last_supported_at: Some(now.clone()),
            last_contradicted_at: existing
                .as_ref()
                .and_then(|item| item.last_contradicted_at.clone()),
            created_at: existing
                .as_ref()
                .map(|item| item.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now,
        })
        .await?;

    Ok(())
}

async fn consolidate_run(storage: &Storage, run: &experience_run::Model) -> Result<()> {
    let tool_names = tool_names_from_value(&run.tool_sequence_json);
    let scope = run.scope.as_str();
    let project_id = run.project_id.as_deref();
    let conversation_id = run.conversation_id.as_deref();
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(request_text) = run.request_text.as_deref() {
        upsert_constraint_from_message(storage, request_text, project_id, conversation_id).await?;
    }

    let is_negative = run.correction_state == "corrected" || run.success_state == "failed";
    if is_negative {
        let related_procedure_key = format!(
            "procedure::{}::{}",
            run.intent_key,
            run.tool_sequence_digest.as_deref().unwrap_or("direct")
        );
        let related_procedure_id = build_item_id(
            "procedure",
            scope,
            project_id,
            conversation_id,
            &related_procedure_key,
        );
        if let Some(mut procedure) = storage.get_experience_item(&related_procedure_id).await? {
            procedure.contradiction_count = procedure.contradiction_count.saturating_add(1);
            procedure.confidence = (procedure.confidence - 0.12).max(0.20);
            procedure.last_contradicted_at = Some(now.clone());
            procedure.updated_at = now.clone();
            storage.upsert_experience_item(&procedure).await?;
        }
    }
    let kind = if is_negative { "lesson" } else { "procedure" };
    let normalized_key = format!(
        "{}::{}::{}",
        kind,
        run.intent_key,
        run.tool_sequence_digest.as_deref().unwrap_or("direct")
    );
    let id = build_item_id(kind, scope, project_id, conversation_id, &normalized_key);
    let existing = storage.get_experience_item(&id).await?;
    let support_count = existing
        .as_ref()
        .map(|item| item.support_count.saturating_add(1))
        .unwrap_or(1);
    let contradiction_count = if is_negative {
        existing
            .as_ref()
            .map(|item| item.contradiction_count)
            .unwrap_or_default()
    } else {
        existing
            .as_ref()
            .map(|item| item.contradiction_count)
            .unwrap_or_default()
    };
    let confidence = if is_negative {
        existing
            .as_ref()
            .map(|item| (item.confidence + 0.05).min(0.94))
            .unwrap_or(0.64)
    } else {
        existing
            .as_ref()
            .map(|item| (item.confidence + 0.08).min(0.98))
            .unwrap_or(0.7)
    };
    let content = if is_negative {
        negative_lesson_summary(run, &tool_names)
    } else {
        positive_procedure_summary(run, &tool_names)
    };
    let steps = suggested_steps_from_tools(&tool_names);
    storage
        .upsert_experience_item(&experience_item::Model {
            id: id.clone(),
            kind: kind.to_string(),
            scope: scope.to_string(),
            project_id: run.project_id.clone(),
            conversation_id: run.conversation_id.clone(),
            title: if is_negative {
                format!("Lesson for {}", run.intent_key)
            } else {
                format!("Procedure for {}", run.intent_key)
            },
            content,
            normalized_key,
            confidence,
            support_count,
            contradiction_count,
            status: "active".to_string(),
            metadata: json!({
                "intent_key": run.intent_key,
                "task_type": run.task_type,
                "tool_sequence_digest": run.tool_sequence_digest,
                "tool_sequence": tool_names,
                "suggested_steps": steps,
                "source_run_id": run.id,
                "polarity": if is_negative { "negative" } else { "positive" },
            }),
            last_supported_at: Some(now.clone()),
            last_contradicted_at: existing
                .as_ref()
                .and_then(|item| item.last_contradicted_at.clone()),
            created_at: existing
                .as_ref()
                .map(|item| item.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now.clone(),
        })
        .await?;

    storage
        .upsert_experience_edge(&experience_edge::Model {
            id: stable_id("edge", &[run.id.as_str(), "derived_from", id.as_str()]),
            source_ref: id.clone(),
            source_kind: "experience_item".to_string(),
            target_ref: run.id.clone(),
            target_kind: "experience_run".to_string(),
            edge_type: "derived_from".to_string(),
            weight: 1.0,
            source_run_id: Some(run.id.clone()),
            metadata: json!({
                "intent_key": run.intent_key,
                "success_state": run.success_state,
                "correction_state": run.correction_state,
            }),
            created_at: now.clone(),
            updated_at: now.clone(),
        })
        .await?;

    storage.mark_experience_run_consolidated(&run.id).await?;
    Ok(())
}

pub async fn run_experience_consolidation(storage: &Storage) -> Result<usize> {
    if !load_learning_enabled(storage).await {
        return Ok(0);
    }
    let cap = load_learning_queue_cap(storage).await as u64;
    let _ = storage
        .finalize_stale_provisional_experience_runs(CORRECTION_WINDOW_MINUTES, cap)
        .await?;
    let runs = storage.list_experience_runs_for_consolidation(cap).await?;
    let mut processed = 0usize;
    for run in runs {
        consolidate_run(storage, &run).await?;
        processed += 1;
    }
    Ok(processed)
}

pub async fn run_pattern_induction(storage: &Storage) -> Result<usize> {
    if !load_learning_enabled(storage).await {
        return Ok(0);
    }
    let procedures = storage
        .list_active_experience_items(
            &["procedure"],
            None,
            None,
            load_learning_queue_cap(storage).await as u64,
        )
        .await?;
    let mut updated = 0usize;
    for procedure in procedures {
        let metadata = procedure.metadata.as_object().cloned().unwrap_or_default();
        let intent_key = metadata
            .get("intent_key")
            .and_then(|value| value.as_str())
            .unwrap_or(procedure.normalized_key.as_str());
        let tool_digest = metadata
            .get("tool_sequence_digest")
            .and_then(|value| value.as_str());
        let pattern_id = build_pattern_id(
            &procedure.scope,
            procedure.project_id.as_deref(),
            procedure.conversation_id.as_deref(),
            intent_key,
            tool_digest,
        );
        let tool_sequence = metadata
            .get("tool_sequence")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let steps_json = metadata
            .get("suggested_steps")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let total = procedure
            .support_count
            .saturating_add(procedure.contradiction_count)
            .max(1);
        let success_rate = procedure.support_count as f64 / total as f64;
        let now = chrono::Utc::now().to_rfc3339();
        storage
            .upsert_procedural_pattern(&procedural_pattern::Model {
                id: pattern_id.clone(),
                intent_key: intent_key.to_string(),
                scope: procedure.scope.clone(),
                project_id: procedure.project_id.clone(),
                conversation_id: procedure.conversation_id.clone(),
                title: procedure.title.clone(),
                trigger_summary: format!(
                    "Use when the request matches `{}` within the current {} scope.",
                    intent_key, procedure.scope
                ),
                summary: procedure.content.clone(),
                tool_sequence_digest: tool_digest.map(|value| value.to_string()),
                steps_json,
                tool_sequence_json: tool_sequence,
                sample_count: procedure.support_count.max(1),
                success_count: procedure.support_count.max(1),
                correction_count: procedure.contradiction_count.max(0),
                success_rate,
                last_validated_at: Some(now.clone()),
                status: if procedure.support_count >= 2 {
                    "active".to_string()
                } else {
                    "draft".to_string()
                },
                metadata: json!({
                    "source_item_id": procedure.id,
                    "task_type": metadata.get("task_type").cloned().unwrap_or(Value::Null),
                }),
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await?;

        storage
            .upsert_experience_edge(&experience_edge::Model {
                id: stable_id(
                    "edge",
                    &[procedure.id.as_str(), "supports", pattern_id.as_str()],
                ),
                source_ref: procedure.id.clone(),
                source_kind: "experience_item".to_string(),
                target_ref: pattern_id.clone(),
                target_kind: "procedural_pattern".to_string(),
                edge_type: "supports".to_string(),
                weight: success_rate.max(0.1),
                source_run_id: None,
                metadata: json!({
                    "intent_key": intent_key,
                }),
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await?;
        updated += 1;
    }
    Ok(updated)
}

fn build_strategy_candidate_profile(
    pattern: &procedural_pattern::Model,
    task_type: &str,
    tool_names: &[String],
) -> crate::core::self_evolve::strategy_runtime::ToolStrategyProfile {
    let mut task_guidance = std::collections::HashMap::new();
    let mut lines = vec![
        format!(
            "When the request matches `{}`, prefer the learned procedure `{}`.",
            task_type, pattern.title
        ),
        "Use this as guidance, not as a hard rule, and adapt when the context clearly differs."
            .to_string(),
    ];
    if !tool_names.is_empty() {
        lines.push(format!(
            "When the environment matches, start with tools in this order: {}.",
            tool_names.join(" -> ")
        ));
    }
    task_guidance.insert(task_type.to_string(), lines);
    crate::core::self_evolve::strategy_runtime::ToolStrategyProfile {
        version: format!(
            "learned-strategy-{}",
            short_hash(&[pattern.id.as_str(), task_type])
        ),
        updated_at: Some(chrono::Utc::now().to_rfc3339()),
        default_guidance: vec![
            "Prefer proven local procedures before improvising a new tool plan.".to_string(),
        ],
        task_guidance,
    }
}

fn canonical_memory_merge_signature(item: &experience_item::Model) -> Option<String> {
    if item.status != "active" {
        return None;
    }
    let content = item.content.trim();
    if content.is_empty() {
        return None;
    }
    let normalized = content
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.len() < 16 {
        return None;
    }
    Some(format!(
        "{}::{}::{}::{}::{}",
        item.kind,
        item.scope,
        item.project_id.as_deref().unwrap_or(""),
        item.conversation_id.as_deref().unwrap_or(""),
        normalized
    ))
}

fn memory_merge_sort_key(item: &experience_item::Model) -> (i32, i32, String, String) {
    (
        item.support_count.max(0),
        (item.confidence * 1000.0) as i32,
        item.updated_at.clone(),
        item.id.clone(),
    )
}

pub async fn run_candidate_generation(storage: &Storage) -> Result<usize> {
    if !load_learning_enabled(storage).await {
        return Ok(0);
    }
    let cap = load_learning_queue_cap(storage).await as u64;
    let patterns = storage.list_candidate_ready_patterns(3, 0.66, cap).await?;
    let mut generated = 0usize;
    for pattern in patterns {
        let steps = pattern
            .steps_json
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(|value| value.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let action_name = candidate_action_name(&pattern);
        let workflow_content = workflow_candidate_markdown(&pattern, &action_name, &steps);
        let workflow_candidate_id = stable_id("candidate", &["workflow", pattern.id.as_str()]);
        let now = chrono::Utc::now().to_rfc3339();
        storage
            .upsert_learning_candidate(&learning_candidate::Model {
                id: workflow_candidate_id,
                candidate_type: "workflow".to_string(),
                subject_key: pattern.id.clone(),
                title: format!("Workflow candidate: {}", pattern.title),
                summary: Some("Generated from repeated successful procedures.".to_string()),
                project_id: pattern.project_id.clone(),
                conversation_id: pattern.conversation_id.clone(),
                pattern_id: Some(pattern.id.clone()),
                evidence_refs: json!([pattern.id]),
                proposed_content: json!({
                    "name": action_name,
                    "content": workflow_content,
                }),
                confidence: pattern.success_rate,
                approval_status: "draft".to_string(),
                review_notes: None,
                reviewed_at: None,
                approved_ref: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await?;
        generated += 1;

        let metadata = pattern.metadata.as_object().cloned().unwrap_or_default();
        let task_type = metadata
            .get("task_type")
            .and_then(|value| value.as_str())
            .unwrap_or("general");
        let tool_names = pattern
            .tool_sequence_json
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(|value| value.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let strategy_profile = build_strategy_candidate_profile(&pattern, task_type, &tool_names);
        let strategy_candidate_id = stable_id("candidate", &["strategy", pattern.id.as_str()]);
        storage
            .upsert_learning_candidate(&learning_candidate::Model {
                id: strategy_candidate_id,
                candidate_type: "strategy".to_string(),
                subject_key: pattern.id.clone(),
                title: format!("Strategy candidate: {}", pattern.title),
                summary: Some("Generated from high-confidence procedural patterns.".to_string()),
                project_id: pattern.project_id.clone(),
                conversation_id: pattern.conversation_id.clone(),
                pattern_id: Some(pattern.id.clone()),
                evidence_refs: json!([pattern.id]),
                proposed_content: serde_json::to_value(strategy_profile).unwrap_or(Value::Null),
                confidence: (pattern.success_rate * 0.92).min(0.98),
                approval_status: "draft".to_string(),
                review_notes: None,
                reviewed_at: None,
                approved_ref: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await?;
        generated += 1;
    }

    let at_risk_procedures = storage
        .list_active_experience_items(&["procedure"], None, None, cap)
        .await?;
    for item in at_risk_procedures
        .into_iter()
        .filter(|item| item.contradiction_count > item.support_count && item.status == "active")
    {
        let now = chrono::Utc::now().to_rfc3339();
        storage
            .upsert_learning_candidate(&learning_candidate::Model {
                id: stable_id("candidate", &["memory_deprecate", item.id.as_str()]),
                candidate_type: "memory_deprecate".to_string(),
                subject_key: item.id.clone(),
                title: format!("Deprecate stale procedure: {}", item.title),
                summary: Some(
                    "The contradiction count has overtaken positive support for this procedure."
                        .to_string(),
                ),
                project_id: item.project_id.clone(),
                conversation_id: item.conversation_id.clone(),
                pattern_id: None,
                evidence_refs: json!([item.id]),
                proposed_content: json!({
                    "item_id": item.id,
                    "next_status": "deprecated",
                }),
                confidence: 0.74,
                approval_status: "draft".to_string(),
                review_notes: None,
                reviewed_at: None,
                approved_ref: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await?;
        generated += 1;
    }

    let mergeable_items = storage
        .list_active_experience_items(
            &["constraint", "personal_fact", "lesson", "procedure"],
            None,
            None,
            cap,
        )
        .await?;
    let mut merge_groups: std::collections::HashMap<String, Vec<experience_item::Model>> =
        std::collections::HashMap::new();
    for item in mergeable_items {
        let Some(signature) = canonical_memory_merge_signature(&item) else {
            continue;
        };
        merge_groups.entry(signature).or_default().push(item);
    }
    for group in merge_groups.into_values() {
        if group.len() < 2 {
            continue;
        }
        let mut sorted = group;
        sorted.sort_by(|a, b| memory_merge_sort_key(b).cmp(&memory_merge_sort_key(a)));
        let target = sorted[0].clone();
        for source in sorted.into_iter().skip(1) {
            if source.id == target.id {
                continue;
            }
            let now = chrono::Utc::now().to_rfc3339();
            storage
                .upsert_learning_candidate(&learning_candidate::Model {
                    id: stable_id("candidate", &["memory_merge", source.id.as_str(), target.id.as_str()]),
                    candidate_type: "memory_merge".to_string(),
                    subject_key: target.id.clone(),
                    title: format!("Merge duplicate memory into {}", target.title),
                    summary: Some(
                        "Two active memories carry substantially the same content and can be merged."
                            .to_string(),
                    ),
                    project_id: target.project_id.clone(),
                    conversation_id: target.conversation_id.clone(),
                    pattern_id: None,
                    evidence_refs: json!([target.id, source.id]),
                    proposed_content: json!({
                        "target_item_id": target.id,
                        "source_item_id": source.id,
                        "reason": "duplicate_content",
                    }),
                    confidence: ((target.confidence + source.confidence) / 2.0).min(0.96),
                    approval_status: "draft".to_string(),
                    review_notes: None,
                    reviewed_at: None,
                    approved_ref: None,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                })
                .await?;
            generated += 1;
        }
    }

    Ok(generated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_intent_key_is_task_aware_and_stable() {
        let key = derive_intent_key(
            "Please fix the Rust bug in the tool execution flow",
            "coding",
        );
        assert!(key.starts_with("coding::"));
        assert!(key.contains("fix"));
        assert!(key.contains("rust"));
    }

    #[test]
    fn candidate_action_name_is_slugged() {
        let pattern = procedural_pattern::Model {
            id: "pattern-123".to_string(),
            intent_key: "coding::fix-tool-bug".to_string(),
            scope: "project".to_string(),
            project_id: None,
            conversation_id: None,
            title: "Fix Tool Bug / Flow".to_string(),
            trigger_summary: String::new(),
            summary: String::new(),
            tool_sequence_digest: None,
            steps_json: Value::Array(Vec::new()),
            tool_sequence_json: Value::Array(Vec::new()),
            sample_count: 3,
            success_count: 3,
            correction_count: 0,
            success_rate: 1.0,
            last_validated_at: None,
            status: "active".to_string(),
            metadata: Value::Null,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let action_name = candidate_action_name(&pattern);
        assert!(action_name.starts_with("learned-fix-tool-bug-flow"));
        assert!(action_name
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'));
    }

    #[test]
    fn describe_user_preference_memory_maps_name_to_personal_fact() {
        let mapped = describe_user_preference_memory("user_name", "Ava")
            .expect("user_name should map to a personal fact");
        assert_eq!(mapped.0, "personal_fact");
        assert!(mapped.2.contains("Ava"));
    }

    #[test]
    fn canonical_memory_merge_signature_normalizes_equivalent_content() {
        let base = experience_item::Model {
            id: "item-1".to_string(),
            kind: "constraint".to_string(),
            scope: "global".to_string(),
            project_id: None,
            conversation_id: None,
            title: "Constraint".to_string(),
            content: "Require explicit approval before side-effecting actions.".to_string(),
            normalized_key: "constraint::one".to_string(),
            confidence: 0.9,
            support_count: 2,
            contradiction_count: 0,
            status: "active".to_string(),
            metadata: Value::Null,
            last_supported_at: None,
            last_contradicted_at: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let mut variant = base.clone();
        variant.id = "item-2".to_string();
        variant.content = "Require explicit approval before side effecting actions".to_string();

        assert_eq!(
            canonical_memory_merge_signature(&base),
            canonical_memory_merge_signature(&variant)
        );
    }
}
