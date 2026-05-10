use super::*;
use anyhow::Context as _;

const DIRECT_CHAT_APPROVAL_TTL_MINS: i64 = 30;
const DIRECT_CHAT_CHAIN_APPROVAL_ACTION: &str = "__agentark_direct_chat_chain__";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedDirectChatChainApproval {
    conversation_id: Option<String>,
    request_channel: String,
    authorization: crate::actions::ActionAuthorizationContext,
    reason: String,
    requested_at: String,
    expires_at: String,
    calls: Vec<DirectChatChainApprovalCall>,
}

fn redact_direct_chat_approval_preview_value(
    key: Option<&str>,
    value: &serde_json::Value,
    depth: usize,
) -> serde_json::Value {
    if key.is_some_and(is_sensitive_tool_call_argument_key) {
        return serde_json::json!("[redacted]");
    }
    if depth >= 3 {
        return match value {
            serde_json::Value::Array(items) => {
                serde_json::json!(format!(
                    "[{} item{}]",
                    items.len(),
                    if items.len() == 1 { "" } else { "s" }
                ))
            }
            serde_json::Value::Object(map) => {
                serde_json::json!(format!(
                    "{{{} field{}}}",
                    map.len(),
                    if map.len() == 1 { "" } else { "s" }
                ))
            }
            serde_json::Value::String(text) => serde_json::json!(safe_truncate(text, 160)),
            other => other.clone(),
        };
    }

    match value {
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys.into_iter().take(16) {
                if let Some(value) = map.get(&key) {
                    out.insert(
                        key.clone(),
                        redact_direct_chat_approval_preview_value(Some(&key), value, depth + 1),
                    );
                }
            }
            if map.len() > 16 {
                out.insert(
                    "_omitted".to_string(),
                    serde_json::json!(map.len().saturating_sub(16)),
                );
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .take(8)
                .map(|item| redact_direct_chat_approval_preview_value(None, item, depth + 1))
                .collect(),
        ),
        serde_json::Value::String(text) => serde_json::json!(safe_truncate(text, 240)),
        other => other.clone(),
    }
}

fn direct_chat_chain_approval_is_expired(request: &PersistedDirectChatChainApproval) -> bool {
    chrono::DateTime::parse_from_rfc3339(&request.expires_at)
        .map(|expires_at| expires_at.with_timezone(&chrono::Utc) <= chrono::Utc::now())
        .unwrap_or(true)
}

fn direct_chat_chain_approval_view(
    id: &str,
    request: &PersistedDirectChatChainApproval,
) -> DirectChatApprovalView {
    let steps = request
        .calls
        .iter()
        .map(|call| DirectChatApprovalStepView {
            action_name: call.action_name.clone(),
            arguments_preview: redact_direct_chat_approval_preview_value(None, &call.arguments, 0),
        })
        .collect::<Vec<_>>();
    let action_name = if request.calls.len() == 1 {
        request.calls[0].action_name.clone()
    } else {
        "action_chain".to_string()
    };
    let arguments_preview = if request.calls.len() == 1 {
        redact_direct_chat_approval_preview_value(None, &request.calls[0].arguments, 0)
    } else {
        serde_json::json!({
            "step_count": request.calls.len(),
            "actions": request
                .calls
                .iter()
                .map(|call| call.action_name.as_str())
                .collect::<Vec<_>>(),
        })
    };
    DirectChatApprovalView {
        id: id.to_string(),
        action_name,
        reason: request.reason.clone(),
        requested_at: request.requested_at.clone(),
        expires_at: request.expires_at.clone(),
        arguments_preview,
        steps,
    }
}

fn direct_chat_approval_choice(
    request: &DirectChatApprovalView,
    decision: &str,
    label: &str,
) -> ClarificationChoice {
    let kind = if request.steps.is_empty() {
        "direct_chat_approval"
    } else {
        "direct_chat_chain_approval"
    };
    ClarificationChoice {
        label: label.to_string(),
        submit_text: format!("{kind}:{decision}:{}", request.id),
        kind: Some(kind.to_string()),
        approval: Some(DirectChatApprovalChoice {
            id: request.id.clone(),
            decision: decision.to_string(),
            action_name: request.action_name.clone(),
            steps: request.steps.clone(),
        }),
    }
}

fn compact_direct_chat_action_result(result: &str) -> String {
    let trimmed = result.trim();
    if trimmed.is_empty() {
        return "The action completed with no output.".to_string();
    }
    safe_truncate(trimmed, 12_000)
}

impl Agent {
    pub(crate) async fn remember_direct_chat_chain_approval(
        &self,
        conversation_id: Option<&str>,
        request_channel: &str,
        calls: &[DirectChatChainApprovalCall],
        authorization: &crate::actions::ActionAuthorizationContext,
        reason: &str,
    ) -> Result<DirectChatApprovalView> {
        if calls.is_empty() {
            anyhow::bail!("Approval request must contain at least one action");
        }
        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::minutes(DIRECT_CHAT_APPROVAL_TTL_MINS);
        let id = uuid::Uuid::new_v4().to_string();
        let request = PersistedDirectChatChainApproval {
            conversation_id: conversation_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            request_channel: request_channel.trim().to_string(),
            authorization: authorization.clone(),
            reason: reason.trim().to_string(),
            requested_at: now.to_rfc3339(),
            expires_at: expires_at.to_rfc3339(),
            calls: calls.to_vec(),
        };
        let serialized = serde_json::to_string(&request)
            .context("failed to serialize direct chat chain approval")?;
        self.storage
            .upsert_approval_request(
                &id,
                DIRECT_CHAT_CHAIN_APPROVAL_ACTION,
                &serialized,
                "direct_chat_chain_explicit_user_approval",
                &request.requested_at,
            )
            .await?;
        Ok(direct_chat_chain_approval_view(&id, &request))
    }

    pub(crate) fn direct_chat_approval_choices(
        &self,
        request: &DirectChatApprovalView,
    ) -> Vec<ClarificationChoice> {
        vec![
            direct_chat_approval_choice(request, "approve", "Approve"),
            direct_chat_approval_choice(request, "reject", "Reject"),
        ]
    }

    async fn load_direct_chat_chain_approval(
        &self,
        approval_id: &str,
    ) -> Result<
        Option<(
            crate::storage::entities::approval_log::Model,
            PersistedDirectChatChainApproval,
        )>,
    > {
        let Some(row) = self.storage.get_approval_request(approval_id).await? else {
            return Ok(None);
        };
        if row.action_name != DIRECT_CHAT_CHAIN_APPROVAL_ACTION {
            return Ok(None);
        }
        let request = serde_json::from_str::<PersistedDirectChatChainApproval>(&row.arguments)
            .with_context(|| format!("failed to decode approval request `{approval_id}`"))?;
        Ok(Some((row, request)))
    }

    pub(crate) async fn reject_direct_chat_any_approval(
        &self,
        approval_id: &str,
    ) -> Result<(DirectChatApprovalView, String)> {
        let Some((row, request)) = self.load_direct_chat_chain_approval(approval_id).await? else {
            anyhow::bail!("Approval request not found or already handled");
        };
        if row.status != "pending" {
            anyhow::bail!("Approval request not found or already handled");
        }
        let view = direct_chat_chain_approval_view(approval_id, &request);
        self.storage
            .resolve_approval_request(approval_id, "denied", "user")
            .await?;
        let response =
            self.filter_direct_chat_approval_response(&format!("Rejected `{}`.", view.action_name));
        self.persist_direct_chat_approval_assistant_message(
            request.conversation_id.as_deref(),
            &response,
        )
        .await?;
        Ok((view, response))
    }

    fn filter_direct_chat_approval_response(&self, response: &str) -> String {
        self.security.filter_output(response).text
    }

    async fn persist_direct_chat_approval_assistant_message(
        &self,
        conversation_id: Option<&str>,
        response: &str,
    ) -> Result<()> {
        let Some(conversation_id) = conversation_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(());
        };
        let response = self.filter_direct_chat_approval_response(response);
        let msg = crate::storage::entities::message::Model {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            role: "assistant".to_string(),
            content: response.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            model_used: Some("approval".to_string()),
            trace_id: None,
        };
        self.encrypted_storage
            .insert_message_encrypted_if_absent(&msg)
            .await?;
        {
            let mut history = self.conversation_history.write().await;
            let conversation_history = history
                .entry(conversation_id.to_string())
                .or_insert_with(Vec::new);
            conversation_history.push(ConversationMessage {
                role: "assistant".to_string(),
                content: response,
                _timestamp: chrono::Utc::now(),
            });
            self.trim_in_memory_conversation_history(conversation_history);
        }
        Ok(())
    }

    pub(crate) async fn approve_direct_chat_any_approval(
        &self,
        approval_id: &str,
    ) -> Result<(DirectChatApprovalView, String)> {
        let Some((row, request)) = self.load_direct_chat_chain_approval(approval_id).await? else {
            anyhow::bail!("Approval request not found or already handled");
        };
        if row.status != "pending" {
            anyhow::bail!("Approval request not found or already handled");
        }
        if direct_chat_chain_approval_is_expired(&request) {
            self.storage
                .resolve_approval_request(approval_id, "expired", "auto_timeout")
                .await?;
            anyhow::bail!("Approval request expired. Ask the agent to run the actions again.");
        }
        let view = direct_chat_chain_approval_view(approval_id, &request);
        let mut authorization = request.authorization.clone();
        authorization.current_turn_is_explicit_approval = true;
        let mut outputs = Vec::new();
        for (index, call) in request.calls.iter().enumerate() {
            let result = match self
                .execute_action_with_hooks(
                    &call.action_name,
                    &call.arguments,
                    &request.request_channel,
                    None,
                    Some(&authorization),
                )
                .await
            {
                Ok(result) => result,
                Err(error) => {
                    self.storage
                        .resolve_approval_request(approval_id, "failed", "system")
                        .await?;
                    anyhow::bail!(
                        "Approved action chain failed while running `{}`: {}",
                        call.action_name,
                        error
                    );
                }
            };
            outputs.push(format!(
                "{}. `{}`\n{}",
                index + 1,
                call.action_name,
                compact_direct_chat_action_result(&result)
            ));
        }
        self.storage
            .resolve_approval_request(approval_id, "approved", "user")
            .await?;
        let response = self.filter_direct_chat_approval_response(&format!(
            "Approved and ran {} action{}.\n\n{}",
            request.calls.len(),
            if request.calls.len() == 1 { "" } else { "s" },
            outputs.join("\n\n")
        ));
        self.persist_direct_chat_approval_assistant_message(
            request.conversation_id.as_deref(),
            &response,
        )
        .await?;
        Ok((view, response))
    }
}
