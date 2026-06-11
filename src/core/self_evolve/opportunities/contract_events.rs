//! Generic operation-contract evidence.
//!
//! Contract events are derived from structured tool/runtime result shapes, not
//! from user phrasing or provider names. They intentionally store only redacted
//! summaries, safe field names, and schema hashes.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub const CONTRACT_EVENTS_METADATA_KEY: &str = "contract_events";
const MAX_CONTRACT_EVENTS_PER_RUN: usize = 8;
const MAX_LIST_ITEMS: usize = 12;
const MAX_TEXT_CHARS: usize = 360;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractEvent {
    #[serde(default)]
    pub source: String,
    pub surface: String,
    pub operation_descriptor: String,
    pub contract_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_hash: Option<String>,
    #[serde(default)]
    pub missing_fields: Vec<String>,
    #[serde(default)]
    pub violations: Vec<String>,
    pub recoverable_by_model: bool,
    pub requires_user_secret: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_instruction: Option<String>,
    pub result_state: String,
}

pub fn append_contract_events(mut metadata: Value, events: &[ContractEvent]) -> Value {
    if events.is_empty() {
        return metadata;
    }
    let new_values = events
        .iter()
        .take(MAX_CONTRACT_EVENTS_PER_RUN)
        .filter_map(|event| serde_json::to_value(event).ok());

    if !metadata.is_object() {
        metadata = Value::Object(Map::new());
    }
    let Some(object) = metadata.as_object_mut() else {
        return metadata;
    };
    let mut existing = object
        .remove(CONTRACT_EVENTS_METADATA_KEY)
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    existing.extend(new_values);
    existing.truncate(MAX_CONTRACT_EVENTS_PER_RUN);
    object.insert(
        CONTRACT_EVENTS_METADATA_KEY.to_string(),
        Value::Array(existing),
    );
    metadata
}

pub fn contract_events_from_metadata(metadata: &Value) -> Vec<ContractEvent> {
    metadata
        .get(CONTRACT_EVENTS_METADATA_KEY)
        .and_then(Value::as_array)
        .map(|events| {
            events
                .iter()
                .filter_map(|event| serde_json::from_value::<ContractEvent>(event.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn contract_events_from_tool_history(tool_history: &[Value]) -> Vec<ContractEvent> {
    let mut events = Vec::new();
    for entry in tool_history {
        let source = entry
            .get("tool")
            .and_then(Value::as_str)
            .map(redact_text)
            .unwrap_or_else(|| "tool".to_string());
        let Some(result) = entry.get("result") else {
            continue;
        };
        collect_contract_events_from_value(result, &source, &mut events, 0);
        if events.len() >= MAX_CONTRACT_EVENTS_PER_RUN {
            events.truncate(MAX_CONTRACT_EVENTS_PER_RUN);
            break;
        }
    }
    dedupe_events(events)
}

pub fn contract_event_identity(event: &ContractEvent) -> String {
    format!(
        "{}::{}::{}::{}",
        event.surface,
        event.contract_kind,
        event.schema_hash.as_deref().unwrap_or("no-schema"),
        event.requires_user_secret
    )
}

fn dedupe_events(events: Vec<ContractEvent>) -> Vec<ContractEvent> {
    let mut seen = std::collections::BTreeSet::new();
    let mut unique = Vec::new();
    for event in events {
        let key = contract_event_identity(&event);
        if seen.insert(key) {
            unique.push(event);
        }
    }
    unique
}

fn collect_contract_events_from_value(
    value: &Value,
    source: &str,
    events: &mut Vec<ContractEvent>,
    depth: usize,
) {
    if depth > 4 || events.len() >= MAX_CONTRACT_EVENTS_PER_RUN {
        return;
    }
    if let Some(object) = value.as_object() {
        if let Some(event) = contract_event_from_object(object, source) {
            events.push(event);
            return;
        }
        for nested in object.values() {
            collect_contract_events_from_value(nested, source, events, depth + 1);
            if events.len() >= MAX_CONTRACT_EVENTS_PER_RUN {
                break;
            }
        }
    } else if let Some(items) = value.as_array() {
        for nested in items {
            collect_contract_events_from_value(nested, source, events, depth + 1);
            if events.len() >= MAX_CONTRACT_EVENTS_PER_RUN {
                break;
            }
        }
    } else if let Some(text) = value.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Value>(text.trim()) {
            collect_contract_events_from_value(&parsed, source, events, depth + 1);
        }
    }
}

fn contract_event_from_object(object: &Map<String, Value>, source: &str) -> Option<ContractEvent> {
    let data = object
        .get("data")
        .and_then(Value::as_object)
        .unwrap_or(object);
    let expected_contract = data
        .get("expected_contract")
        .or_else(|| object.get("expected_contract"));
    let violations_value = data.get("violations").or_else(|| object.get("violations"));
    let credential_request = data
        .get("credential_request")
        .or_else(|| object.get("credential_request"));
    let assistant_instruction = data
        .get("assistant_instruction")
        .or_else(|| object.get("assistant_instruction"))
        .and_then(Value::as_str)
        .map(redact_text)
        .filter(|value| !value.is_empty());
    let has_contract_signal = expected_contract.is_some()
        || credential_request.is_some()
        || violations_value
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
        || assistant_instruction.is_some();
    if !has_contract_signal {
        return None;
    }

    let status = object
        .get("status")
        .or_else(|| data.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("failed");
    let result_state = result_state_from_status(status);
    let requires_user_secret = credential_request.is_some()
        || data
            .get("requires_user_secret")
            .or_else(|| data.get("secure_input_required"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let recoverable_by_model = data
        .get("recoverable_by_model")
        .or_else(|| object.get("recoverable_by_model"))
        .and_then(Value::as_bool)
        .unwrap_or(!requires_user_secret && result_state != "served");
    let contract_kind =
        infer_contract_kind(expected_contract, violations_value, requires_user_secret);
    let surface = infer_surface(object, data, requires_user_secret);
    let schema_hash = expected_contract.map(stable_json_hash);
    let missing_fields = infer_missing_fields(expected_contract, violations_value);
    let violations = safe_violations(violations_value);
    Some(ContractEvent {
        source: source.to_string(),
        surface: surface.clone(),
        operation_descriptor: format!("{} {}", surface, contract_kind),
        contract_kind,
        schema_hash,
        missing_fields,
        violations,
        recoverable_by_model,
        requires_user_secret,
        assistant_instruction,
        result_state,
    })
}

fn infer_surface(
    object: &Map<String, Value>,
    data: &Map<String, Value>,
    requires_user_secret: bool,
) -> String {
    if requires_user_secret {
        return "integration_setup".to_string();
    }
    let tool = object
        .get("tool")
        .or_else(|| data.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let integration_type = data
        .get("integration_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if integration_type == "custom_api" || tool == "custom_api" {
        "custom_api".to_string()
    } else if tool == "http_request" {
        "external_action".to_string()
    } else if data.get("expected_contract").is_some() {
        "tool_call".to_string()
    } else {
        "runtime_capability".to_string()
    }
}

fn infer_contract_kind(
    expected_contract: Option<&Value>,
    violations: Option<&Value>,
    requires_user_secret: bool,
) -> String {
    if requires_user_secret {
        return "credential_collection".to_string();
    }
    if let Some(contract) = expected_contract {
        if contract.pointer("/required/arguments").is_some() || contract.get("substrate").is_some()
        {
            return "required_arguments".to_string();
        }
        if contract.get("required_when_body_bearing").is_some()
            || contract.get("normalization").is_some()
        {
            return "operation_envelope".to_string();
        }
        if contract.get("required").is_some() {
            return "request_body_schema".to_string();
        }
    }
    if violations
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        return "required_arguments".to_string();
    }
    "operation_contract".to_string()
}

fn infer_missing_fields(
    expected_contract: Option<&Value>,
    violations: Option<&Value>,
) -> Vec<String> {
    let mut fields = std::collections::BTreeSet::new();
    if let Some(required) = expected_contract
        .and_then(|contract| contract.get("required"))
        .and_then(Value::as_object)
    {
        fields.extend(required.keys().map(|key| safe_field_name(key)));
    }
    if let Some(required) = expected_contract
        .and_then(|contract| contract.get("required_when_body_bearing"))
        .and_then(Value::as_object)
    {
        fields.extend(required.keys().map(|key| safe_field_name(key)));
    }
    if let Some(items) = violations.and_then(Value::as_array) {
        for item in items {
            if let Some(code) = item.get("code").and_then(Value::as_str) {
                fields.insert(safe_field_name(code));
            }
        }
    }
    fields
        .into_iter()
        .filter(|field| !field.is_empty())
        .take(MAX_LIST_ITEMS)
        .collect()
}

fn safe_violations(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("code")
                        .and_then(Value::as_str)
                        .or_else(|| item.as_str())
                        .map(safe_field_name)
                })
                .filter(|value| !value.is_empty())
                .take(MAX_LIST_ITEMS)
                .collect()
        })
        .unwrap_or_default()
}

fn result_state_from_status(status: &str) -> String {
    let normalized = status.trim().to_ascii_lowercase();
    if normalized.starts_with("needs_") || normalized == "approval_required" {
        "failed".to_string()
    } else if matches!(
        normalized.as_str(),
        "completed" | "success" | "succeeded" | "ok"
    ) {
        "served".to_string()
    } else {
        "failed".to_string()
    }
}

fn safe_field_name(value: &str) -> String {
    value
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                Some(ch.to_ascii_lowercase())
            } else if ch.is_whitespace() {
                Some('_')
            } else {
                None
            }
        })
        .take(96)
        .collect()
}

fn redact_text(value: &str) -> String {
    let redacted = crate::security::redact_secret_input(value).text;
    scrub_token_like_fragments(&redacted)
        .chars()
        .take(MAX_TEXT_CHARS)
        .collect()
}

fn scrub_token_like_fragments(value: &str) -> String {
    value
        .split_whitespace()
        .map(|part| {
            let trimmed = part.trim_matches(|ch: char| ch.is_ascii_punctuation());
            if looks_like_secret_fragment(trimmed) {
                part.replace(trimmed, "[REDACTED]")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_secret_fragment(value: &str) -> bool {
    let len = value.chars().count();
    if len < 12 {
        return false;
    }
    let allowed = value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | '=' | ':' | '+')
    });
    if !allowed {
        return false;
    }
    let separator_count = value
        .chars()
        .filter(|ch| matches!(ch, '-' | '_' | '.' | '/' | '=' | ':' | '+'))
        .count();
    let has_digit = value.chars().any(|ch| ch.is_ascii_digit());
    let lower = value.to_ascii_lowercase();
    let secret_named = [
        "secret",
        "token",
        "password",
        "api_key",
        "apikey",
        "credential",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    has_digit || separator_count >= 2 || secret_named
}

fn stable_json_hash(value: &Value) -> String {
    let mut hasher = Sha256::new();
    let rendered = serde_json::to_string(value).unwrap_or_default();
    hasher.update(rendered.as_bytes());
    let digest = hex::encode(hasher.finalize());
    digest[..24].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_request_contract_event_without_raw_body_or_secret() {
        let history = vec![json!({
            "tool": "custom_api",
            "result": {
                "tool": "custom_api",
                "status": "needs_arguments",
                "data": {
                    "violations": [{"code": "missing_request_body", "message": "body required"}],
                    "expected_contract": {
                        "required_when_body_bearing": {
                            "method": "method",
                            "headers": "headers",
                            "body": "body"
                        }
                    },
                    "assistant_instruction": "Use token sk-live-secret in a body"
                }
            }
        })];

        let events = contract_events_from_tool_history(&history);
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.surface, "custom_api");
        assert_eq!(event.contract_kind, "operation_envelope");
        assert_eq!(event.result_state, "failed");
        assert!(event.missing_fields.iter().any(|field| field == "body"));
        assert!(event.schema_hash.is_some());
        assert!(!serde_json::to_string(event)
            .unwrap()
            .contains("sk-live-secret"));
    }

    #[test]
    fn credential_request_becomes_secure_setup_event() {
        let history = vec![json!({
            "tool": "custom_api",
            "result": {
                "status": "needs_credentials",
                "data": {
                    "credential_request": {
                        "secure_input_required": true,
                        "fields": [{"key": "secret"}]
                    },
                    "assistant_instruction": "Use secure credential setup."
                }
            }
        })];

        let events = contract_events_from_tool_history(&history);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].surface, "integration_setup");
        assert_eq!(events[0].contract_kind, "credential_collection");
        assert!(events[0].requires_user_secret);
        assert!(!events[0].recoverable_by_model);
    }

    #[test]
    fn appends_events_to_metadata_without_overwriting_existing_fields() {
        let event = ContractEvent {
            source: "tool".to_string(),
            surface: "tool_call".to_string(),
            operation_descriptor: "tool_call required_arguments".to_string(),
            contract_kind: "required_arguments".to_string(),
            schema_hash: Some("abc".to_string()),
            missing_fields: vec!["arguments".to_string()],
            violations: vec!["invalid_arguments_envelope".to_string()],
            recoverable_by_model: true,
            requires_user_secret: false,
            assistant_instruction: None,
            result_state: "failed".to_string(),
        };
        let metadata = append_contract_events(json!({"existing": true}), &[event.clone()]);
        assert_eq!(metadata["existing"], true);
        assert_eq!(contract_events_from_metadata(&metadata), vec![event]);
    }
}
