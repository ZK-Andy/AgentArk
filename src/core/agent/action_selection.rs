use super::*;
use crate::actions::ActionDef;

pub(super) fn action_is_read_only(action: &ActionDef) -> bool {
    matches!(
        action.planner_metadata().side_effect_level,
        PlannerSideEffectLevel::None
    )
}

pub(super) fn action_is_read_only_knowledge_action(action: &ActionDef) -> bool {
    let metadata = action.planner_metadata();
    action_is_read_only(action)
        && matches!(
            metadata.role,
            PlannerActionRole::DataSource | PlannerActionRole::Inspection
        )
        && matches!(
            metadata.integration_class,
            PlannerIntegrationClass::Search
                | PlannerIntegrationClass::Network
                | PlannerIntegrationClass::Analytics
                | PlannerIntegrationClass::Internal
                | PlannerIntegrationClass::Workspace
                | PlannerIntegrationClass::Filesystem
        )
}

pub(super) fn format_recent_dialogue_for_fast_path(
    history: &[ConversationMessage],
) -> Option<String> {
    let lines = history
        .iter()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .filter_map(|message| {
            let content = message.content.trim();
            if content.is_empty() {
                return None;
            }
            Some(format!("{}: {}", message.role, safe_truncate(content, 240)))
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn direct_reply_action_scope_query(message: &str, request_hints: &RequestExecutionHints) -> String {
    let mut parts = Vec::new();
    let trimmed = message.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
    if let Some(routing) = request_hints.routing.as_ref() {
        parts.extend(
            routing
                .semantic_queries
                .iter()
                .chain(routing.required_capabilities.iter())
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
        );
        for goal in &routing.goals {
            for value in [
                goal.intent_summary.as_str(),
                goal.capability_query.as_str(),
                goal.expected_outcome.as_str(),
            ] {
                let value = value.trim();
                if !value.is_empty() {
                    parts.push(value.to_string());
                }
            }
        }
    }

    let mut seen = HashSet::new();
    parts
        .into_iter()
        .filter(|value| seen.insert(value.to_ascii_lowercase()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn routing_signal_has_read_only_retrieval_need(
    routing: &crate::security::intent_classifier::InboundRoutingSignal,
) -> bool {
    routing.agentark_capabilities_expected
        || routing.agentark_manual_expected
        || routing.live_state_expected
        || routing.external_info_expected
}

fn connected_read_only_action_score(
    scope_query: &str,
    action: &ActionDef,
    semantic_scores: &HashMap<String, f32>,
) -> f32 {
    let mut lexical = crate::core::capability_router::score_action_intent(scope_query, action);
    for part in scope_query
        .lines()
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        lexical = lexical.max(crate::core::capability_router::score_action_intent(
            part, action,
        ));
    }
    lexical
        .max(
            semantic_scores
                .get(&action.name)
                .copied()
                .unwrap_or_default(),
        )
        .clamp(0.0, 1.0)
}

impl Agent {
    pub(super) async fn direct_reply_should_yield_to_read_only_action(
        &self,
        message: &str,
        request_hints: &RequestExecutionHints,
    ) -> bool {
        let actions = match self.load_action_catalog_actions().await {
            Ok(actions) => actions,
            Err(error) => {
                tracing::debug!(
                    "Direct-reply action gate skipped because action catalog failed: {}",
                    error
                );
                return false;
            }
        };
        let authorization = crate::actions::ActionAuthorizationContext {
            principal: request_hints.caller_principal.clone(),
            surface: request_hints.execution_surface.clone(),
            direct_user_intent: request_hints.direct_user_intent,
            current_turn_is_explicit_approval: false,
            agent_name: None,
            agent_access_scope: None,
            capability_context_id: None,
        };
        let authorized = self
            .authorize_agent_loop_actions_for_turn(&actions, &authorization)
            .await;
        if authorized.is_empty() {
            return false;
        }
        let scope_query = direct_reply_action_scope_query(message, request_hints);
        if scope_query.trim().is_empty() {
            return false;
        }
        let scores = self
            .semantic_action_scores_for_agent_loop(&scope_query, &authorized)
            .await;

        let routing_has_retrieval_need = request_hints
            .routing
            .as_ref()
            .map(routing_signal_has_read_only_retrieval_need)
            .unwrap_or(false);
        let mut scored = authorized
            .iter()
            .filter(|action| action_is_read_only_knowledge_action(action))
            .map(|action| {
                (
                    action,
                    connected_read_only_action_score(&scope_query, action, &scores),
                )
            })
            .filter(|(_, score)| *score > 0.0)
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let Some((best_action, best_score)) = scored.first().copied() else {
            return false;
        };
        let should_yield = routing_has_retrieval_need;
        if should_yield {
            tracing::info!(
                action = %best_action.name,
                score = best_score,
                routing_has_retrieval_need,
                "Direct conversation path yielded to structurally routed read-only action"
            );
        }
        should_yield
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_yield_requires_structured_retrieval_need() {
        let mut routing = crate::security::intent_classifier::InboundRoutingSignal {
            current_answer_expected: true,
            ..Default::default()
        };

        assert!(!routing_signal_has_read_only_retrieval_need(&routing));

        routing.live_state_expected = true;
        assert!(routing_signal_has_read_only_retrieval_need(&routing));
    }

    #[test]
    fn routed_retrieval_need_is_structural_not_score_gated() {
        let routing = crate::security::intent_classifier::InboundRoutingSignal {
            current_answer_expected: true,
            live_state_expected: true,
            ..Default::default()
        };

        assert!(routing_signal_has_read_only_retrieval_need(&routing));
    }
}
