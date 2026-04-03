use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanPromptMode {
    ChatExecution,
    TaskAutomation,
    GoalLoop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanStepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: usize,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PlanStepStatus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub plan_id: String,
    pub revision: u32,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub steps: Vec<PlanStep>,
}

const DEFAULT_MAX_PLAN_STEPS: usize = 8;
pub const DEFAULT_MAX_ACTIONS_FOR_PLAN: usize = 8;

fn extract_json(text: &str) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .or_else(|| {
            let start = text.find('{').or_else(|| text.find('['))?;
            let end = text.rfind('}').or_else(|| text.rfind(']'))?;
            serde_json::from_str::<serde_json::Value>(&text[start..=end]).ok()
        })
}

fn action_catalog_for_prompt(actions: &[crate::actions::ActionDef]) -> Vec<serde_json::Value> {
    actions
        .iter()
        .map(|action| {
            serde_json::json!({
                "name": action.name,
                "description": action.description,
                "input_schema": action.input_schema,
                "planner_metadata": action.planner_metadata(),
            })
        })
        .collect()
}

fn allowed_action_names(actions: &[crate::actions::ActionDef]) -> HashSet<String> {
    actions
        .iter()
        .map(|action| action.name.trim().to_ascii_lowercase())
        .collect()
}

fn normalize_status(value: &serde_json::Value) -> Option<PlanStepStatus> {
    match value.as_str()?.trim().to_ascii_lowercase().as_str() {
        "pending" => Some(PlanStepStatus::Pending),
        "running" => Some(PlanStepStatus::Running),
        "completed" => Some(PlanStepStatus::Completed),
        "failed" => Some(PlanStepStatus::Failed),
        "skipped" => Some(PlanStepStatus::Skipped),
        _ => None,
    }
}

fn normalize_action_name(
    value: Option<&serde_json::Value>,
    allowed_actions: &HashSet<String>,
) -> Option<String> {
    let raw = value?.as_str()?.trim();
    if raw.is_empty() {
        return None;
    }
    let normalized = raw.to_ascii_lowercase();
    if allowed_actions.contains(&normalized) {
        Some(raw.to_string())
    } else {
        None
    }
}

fn normalize_arguments(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    let value = value?.clone();
    if value.is_object() {
        Some(value)
    } else {
        None
    }
}

fn normalize_plan_step(
    index: usize,
    value: &serde_json::Value,
    allowed_actions: &HashSet<String>,
    include_status: bool,
) -> Option<PlanStep> {
    let record = value.as_object()?;
    let title = record
        .get("title")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Step")
        .to_string();
    let description = record
        .get("description")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or("")
        .to_string();

    let mut action = normalize_action_name(record.get("action"), allowed_actions);
    let mut tool_hint = normalize_action_name(record.get("tool_hint"), allowed_actions);
    if action.is_none() {
        action = tool_hint.clone();
    }
    if tool_hint.is_none() {
        tool_hint = action.clone();
    }

    let status = if include_status {
        record.get("status").and_then(normalize_status)
    } else {
        None
    };

    Some(PlanStep {
        id: index + 1,
        title,
        description,
        action,
        arguments: normalize_arguments(record.get("arguments")),
        tool_hint,
        status,
    })
}

fn normalize_plan_steps(
    raw_steps: &[serde_json::Value],
    actions: &[crate::actions::ActionDef],
    include_status: bool,
) -> Vec<PlanStep> {
    let allowed = allowed_action_names(actions);
    raw_steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| normalize_plan_step(index, step, &allowed, include_status))
        .take(DEFAULT_MAX_PLAN_STEPS)
        .collect()
}

pub fn create_plan(
    summary: impl Into<String>,
    steps: Vec<PlanStep>,
    plan_id: Option<String>,
    revision: u32,
) -> ExecutionPlan {
    ExecutionPlan {
        plan_id: plan_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        revision,
        summary: summary.into().trim().to_string(),
        steps,
    }
}

pub fn build_action_selector_prompt(
    request: &str,
    refinement: Option<&str>,
    available_actions: &[crate::actions::ActionDef],
) -> (String, String) {
    let system = format!(
        "You are a task planner for an AI agent.\n\
Return ONLY valid JSON.\n\n\
Output schema:\n\
{{\n  \"summary\": \"short summary\",\n  \"needed_actions\": [\"action_name\"]\n}}\n\n\
Rules:\n\
- Use only the provided actions.\n\
- Keep the list minimal and relevant.\n\
- Select at most {} actions.\n\
- Do not include actions that are only for presenting the final answer.\n",
        DEFAULT_MAX_ACTIONS_FOR_PLAN
    );

    let mut user = format!(
        "Request:\n{}\n\nAvailable actions:\n{}",
        request.trim(),
        serde_json::to_string_pretty(
            &available_actions
                .iter()
                .map(|action| {
                    serde_json::json!({
                        "name": action.name,
                        "description": action.description,
                        "planner_metadata": action.planner_metadata(),
                    })
                })
                .collect::<Vec<_>>()
        )
        .unwrap_or_default()
    );

    if let Some(refinement) = refinement.map(str::trim).filter(|value| !value.is_empty()) {
        user.push_str("\n\nRefinement:\n");
        user.push_str(refinement);
    }

    (system, user)
}

pub fn parse_action_selection(
    raw: &str,
    available_actions: &[crate::actions::ActionDef],
    max_actions: usize,
) -> Vec<String> {
    let allowed = allowed_action_names(available_actions);
    let Some(value) = extract_json(raw) else {
        return Vec::new();
    };
    value
        .get("needed_actions")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let raw = item.as_str()?.trim();
                    if raw.is_empty() {
                        return None;
                    }
                    let normalized = raw.to_ascii_lowercase();
                    if allowed.contains(&normalized) {
                        Some(raw.to_string())
                    } else {
                        None
                    }
                })
                .take(max_actions)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub fn shortlist_actions(
    available_actions: &[crate::actions::ActionDef],
    selected_names: &[String],
    max_actions: usize,
) -> Vec<crate::actions::ActionDef> {
    let mut scoped = available_actions
        .iter()
        .filter(|action| selected_names.iter().any(|name| name == &action.name))
        .take(max_actions)
        .cloned()
        .collect::<Vec<_>>();

    if scoped.is_empty() {
        scoped = available_actions
            .iter()
            .take(max_actions)
            .cloned()
            .collect::<Vec<_>>();
    }

    scoped
}

pub fn build_plan_prompt(
    request: &str,
    refinement: Option<&str>,
    available_actions: &[crate::actions::ActionDef],
    mode: PlanPromptMode,
    current_plan: Option<&ExecutionPlan>,
) -> (String, String) {
    let mode_line = match mode {
        PlanPromptMode::ChatExecution => {
            "This plan is for live execution. Prefer concrete, observable steps that map directly to available actions."
        }
        PlanPromptMode::TaskAutomation => {
            "This plan is for a stored task/automation. Prefer steps with runnable action names and arguments."
        }
        PlanPromptMode::GoalLoop => {
            "This plan is for an ongoing goal loop. Prefer compact, reusable execution steps with runnable action names and arguments."
        }
    };

    let system = format!(
        "You are a task planner for an AI agent.\n\
Return ONLY valid JSON.\n\n\
Output schema:\n\
{{\n  \"summary\": \"short summary\",\n  \"steps\": [\n    {{\n      \"title\": \"short step title\",\n      \"description\": \"one sentence\",\n      \"action\": \"action_name or null\",\n      \"arguments\": {{}} ,\n      \"tool_hint\": \"action_name or null\"\n    }}\n  ]\n}}\n\n\
Rules:\n\
- Use only the provided actions.\n\
- 1-{} steps maximum.\n\
- Each step should be one logical action, not a sub-plan.\n\
- Do not add a separate final step just to summarize or present the result.\n\
- If a step maps directly to an available action, set both `action` and `tool_hint` to that exact action name.\n\
- If a step does not directly map to an available action, set both `action` and `tool_hint` to null.\n\
- Keep descriptions concrete and avoid filler.\n\
- Do not include `status`, `plan_id`, or `revision` in the response.\n\
- {}\n",
        DEFAULT_MAX_PLAN_STEPS,
        mode_line
    );

    let mut user = format!(
        "Request:\n{}\n\nAvailable actions:\n{}",
        request.trim(),
        serde_json::to_string_pretty(&action_catalog_for_prompt(available_actions))
            .unwrap_or_default()
    );

    if let Some(refinement) = refinement.map(str::trim).filter(|value| !value.is_empty()) {
        user.push_str("\n\nRefinement:\n");
        user.push_str(refinement);
    }

    if let Some(current_plan) = current_plan {
        user.push_str("\n\nCurrent execution plan:\n");
        user.push_str(&serde_json::to_string_pretty(current_plan).unwrap_or_default());
        user.push_str(
            "\n\nIf the plan must change, return a full replacement plan for the remaining work only.",
        );
    }

    (system, user)
}

pub fn parse_plan_from_llm_content(
    raw: &str,
    available_actions: &[crate::actions::ActionDef],
    plan_id: Option<String>,
    revision: u32,
    include_status: bool,
) -> Option<ExecutionPlan> {
    let value = extract_json(raw)?;
    parse_plan_from_value(&value, available_actions, plan_id, revision, include_status)
}

pub fn parse_plan_from_value(
    value: &serde_json::Value,
    available_actions: &[crate::actions::ActionDef],
    plan_id: Option<String>,
    revision: u32,
    include_status: bool,
) -> Option<ExecutionPlan> {
    let (summary, raw_steps) = if let Some(array) = value.as_array() {
        (String::new(), array.clone())
    } else {
        let summary = value
            .get("summary")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        let steps = value
            .get("steps")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        (summary, steps)
    };

    let steps = normalize_plan_steps(&raw_steps, available_actions, include_status);
    if steps.is_empty() {
        return None;
    }

    Some(create_plan(summary, steps, plan_id, revision))
}

pub fn prepare_plan_for_execution(plan: &ExecutionPlan) -> ExecutionPlan {
    let steps = plan
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| PlanStep {
            id: index + 1,
            title: step.title.clone(),
            description: step.description.clone(),
            action: step.action.clone(),
            arguments: step.arguments.clone(),
            tool_hint: step.tool_hint.clone(),
            status: Some(PlanStepStatus::Pending),
        })
        .collect();

    create_plan(
        plan.summary.clone(),
        steps,
        Some(plan.plan_id.clone()),
        plan.revision,
    )
}

pub fn next_revision_plan(
    current_plan: &ExecutionPlan,
    replacement: &ExecutionPlan,
) -> ExecutionPlan {
    let mut steps = current_plan
        .steps
        .iter()
        .filter(|step| {
            matches!(
                step.status,
                Some(PlanStepStatus::Completed)
                    | Some(PlanStepStatus::Failed)
                    | Some(PlanStepStatus::Skipped)
            )
        })
        .cloned()
        .collect::<Vec<_>>();

    steps.extend(replacement.steps.iter().map(|step| PlanStep {
        id: 0,
        title: step.title.clone(),
        description: step.description.clone(),
        action: step.action.clone(),
        arguments: step.arguments.clone(),
        tool_hint: step.tool_hint.clone(),
        status: Some(PlanStepStatus::Pending),
    }));

    for (index, step) in steps.iter_mut().enumerate() {
        step.id = index + 1;
    }

    create_plan(
        if replacement.summary.trim().is_empty() {
            current_plan.summary.clone()
        } else {
            replacement.summary.clone()
        },
        steps,
        Some(current_plan.plan_id.clone()),
        current_plan.revision.saturating_add(1),
    )
}

pub fn active_plan_step(plan: &ExecutionPlan) -> Option<&PlanStep> {
    plan.steps
        .iter()
        .find(|step| step.status == Some(PlanStepStatus::Running))
        .or_else(|| {
            plan.steps
                .iter()
                .find(|step| step.status == Some(PlanStepStatus::Pending))
        })
}

pub fn render_plan_for_system_prompt(plan: &ExecutionPlan) -> String {
    let summary = if plan.summary.trim().is_empty() {
        "Execution plan"
    } else {
        plan.summary.trim()
    };
    let steps = plan
        .steps
        .iter()
        .map(|step| format!("{}. {} - {}", step.id, step.title, step.description))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "## Execution Plan\nSummary: {}\nPlan ID: {}\nRevision: {}\nFollow this plan step-by-step unless you emit a full replacement plan revision first.\n{}\nDo not narrate new future steps that are not in this plan.",
        summary,
        plan.plan_id,
        plan.revision,
        steps
    )
}

pub fn render_plan_followup_context(plan: &ExecutionPlan) -> String {
    let active = active_plan_step(plan)
        .map(|step| format!("{}: {}", step.id, step.title))
        .unwrap_or_else(|| "none".to_string());
    format!(
        "Current execution plan (plan_id={}, revision={}):\n{}\n\nActive step: {}\nRules:\n- Use the current plan as the source of truth.\n- Do not narrate new future steps unless you first return a full replacement plan JSON object for the remaining work.\n- If you revise the plan, return ONLY the replacement plan JSON object and do not call tools in the same response.",
        plan.plan_id,
        plan.revision,
        serde_json::to_string_pretty(plan).unwrap_or_default(),
        active
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{ActionDef, ActionSource};

    fn action(name: &str) -> ActionDef {
        ActionDef {
            name: name.to_string(),
            description: format!("{} description", name),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({}),
            capabilities: vec![],
            sandbox_mode: None,
            source: ActionSource::System,
            file_path: None,
            authorization: Default::default(),
        }
    }

    fn plan_step(
        id: usize,
        title: &str,
        tool_hint: Option<&str>,
        status: Option<PlanStepStatus>,
    ) -> PlanStep {
        PlanStep {
            id,
            title: title.to_string(),
            description: format!("{} description", title),
            action: tool_hint.map(str::to_string),
            arguments: None,
            tool_hint: tool_hint.map(str::to_string),
            status,
        }
    }

    #[test]
    fn parse_plan_from_canonical_object_keeps_summary_and_known_actions() {
        let actions = vec![action("file_write"), action("app_deploy")];
        let parsed = parse_plan_from_llm_content(
            r#"
            {
              "summary": "Ship the dashboard",
              "steps": [
                {
                  "title": "Write files",
                  "description": "Create the source files",
                  "action": "file_write",
                  "arguments": {"path":"/tmp/demo"},
                  "tool_hint": "file_write"
                },
                {
                  "title": "Deploy",
                  "description": "Launch the app",
                  "action": "app_deploy",
                  "tool_hint": "app_deploy"
                }
              ]
            }
            "#,
            &actions,
            Some("plan-1".to_string()),
            1,
            false,
        )
        .expect("canonical plan should parse");

        assert_eq!(parsed.plan_id, "plan-1");
        assert_eq!(parsed.revision, 1);
        assert_eq!(parsed.summary, "Ship the dashboard");
        assert_eq!(parsed.steps.len(), 2);
        assert_eq!(parsed.steps[0].action.as_deref(), Some("file_write"));
        assert_eq!(
            parsed.steps[0]
                .arguments
                .as_ref()
                .and_then(|value| value.get("path"))
                .and_then(|value| value.as_str()),
            Some("/tmp/demo")
        );
        assert_eq!(parsed.steps[1].tool_hint.as_deref(), Some("app_deploy"));
    }

    #[test]
    fn parse_plan_from_legacy_array_drops_unknown_actions() {
        let actions = vec![action("http_get")];
        let parsed = parse_plan_from_llm_content(
            r#"
            [
              {"title":"Check health","description":"Verify the site","tool_hint":"http_get"},
              {"title":"Notify","description":"Send an update","tool_hint":"email_send"}
            ]
            "#,
            &actions,
            Some("plan-legacy".to_string()),
            2,
            false,
        )
        .expect("legacy plan array should still parse");

        assert_eq!(parsed.summary, "");
        assert_eq!(parsed.steps.len(), 2);
        assert_eq!(parsed.steps[0].tool_hint.as_deref(), Some("http_get"));
        assert_eq!(parsed.steps[1].tool_hint, None);
        assert_eq!(parsed.steps[1].action, None);
    }

    #[test]
    fn parse_plan_rejects_malformed_or_empty_payloads() {
        let actions = vec![action("file_write")];
        assert!(parse_plan_from_llm_content("not json", &actions, None, 1, false).is_none());
        assert!(parse_plan_from_llm_content(
            r#"{"summary":"No steps","steps":[]}"#,
            &actions,
            None,
            1,
            false
        )
        .is_none());
    }

    #[test]
    fn next_revision_plan_preserves_completed_steps_and_appends_remaining_work() {
        let current = ExecutionPlan {
            plan_id: "plan-42".to_string(),
            revision: 3,
            summary: "Current summary".to_string(),
            steps: vec![
                plan_step(
                    1,
                    "Inspect",
                    Some("file_read"),
                    Some(PlanStepStatus::Completed),
                ),
                plan_step(
                    2,
                    "Patch",
                    Some("file_write"),
                    Some(PlanStepStatus::Running),
                ),
                plan_step(3, "Verify", Some("http_get"), Some(PlanStepStatus::Pending)),
            ],
        };
        let replacement = ExecutionPlan {
            plan_id: "ignored".to_string(),
            revision: 99,
            summary: "Revised summary".to_string(),
            steps: vec![
                plan_step(1, "Patch with new approach", Some("file_write"), None),
                plan_step(2, "Verify again", Some("http_get"), None),
            ],
        };

        let revised = next_revision_plan(&current, &replacement);

        assert_eq!(revised.plan_id, "plan-42");
        assert_eq!(revised.revision, 4);
        assert_eq!(revised.summary, "Revised summary");
        assert_eq!(revised.steps.len(), 3);
        assert_eq!(revised.steps[0].title, "Inspect");
        assert_eq!(revised.steps[0].status, Some(PlanStepStatus::Completed));
        assert_eq!(revised.steps[1].title, "Patch with new approach");
        assert_eq!(revised.steps[1].status, Some(PlanStepStatus::Pending));
        assert_eq!(revised.steps[2].title, "Verify again");
        assert_eq!(revised.steps[2].status, Some(PlanStepStatus::Pending));
    }
}
