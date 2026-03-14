use crate::core::llm::LlmProvider;
use crate::core::orchestra::SubAgentType;
use crate::core::swarm::{AgentCapability, SpecialistConfig};
use crate::storage::entities::swarm_agent;

fn capability_from_text(text: &str) -> Option<AgentCapability> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(AgentCapability {
        name: trimmed.to_string(),
        description: trimmed.to_string(),
        keywords: trimmed
            .split_whitespace()
            .map(|word| word.to_ascii_lowercase())
            .collect(),
    })
}

pub fn capability_strings_to_models(values: &[String]) -> Vec<AgentCapability> {
    values
        .iter()
        .filter_map(|value| capability_from_text(value))
        .collect()
}

pub fn capability_models_to_strings(values: &[AgentCapability]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.description.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn parse_agent_type(agent_type: &str, custom_instructions: Option<&str>) -> SubAgentType {
    match agent_type.trim().to_ascii_lowercase().as_str() {
        "researcher" => SubAgentType::Researcher,
        "coder" => SubAgentType::Coder,
        "analyst" => SubAgentType::Analyst,
        "writer" => SubAgentType::Writer,
        "validator" => SubAgentType::Validator,
        "planner" => SubAgentType::Planner,
        other => SubAgentType::Custom {
            name: if other.is_empty() {
                "Custom".to_string()
            } else {
                agent_type.trim().to_string()
            },
            instructions: custom_instructions.unwrap_or_default().to_string(),
        },
    }
}

pub fn parse_capabilities(raw: &str) -> Vec<AgentCapability> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if let Ok(values) = serde_json::from_str::<Vec<String>>(trimmed) {
        return capability_strings_to_models(&values);
    }

    if let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(trimmed) {
        return values
            .iter()
            .filter_map(|value| {
                if let Some(text) = value.as_str() {
                    return capability_from_text(text);
                }
                let name = value
                    .get("name")
                    .and_then(|item| item.as_str())
                    .or_else(|| value.get("description").and_then(|item| item.as_str()))
                    .unwrap_or_default();
                capability_from_text(name)
            })
            .collect();
    }

    trimmed
        .split(',')
        .filter_map(capability_from_text)
        .collect()
}

pub fn parse_llm_provider(raw: &str, fallback: &LlmProvider) -> LlmProvider {
    serde_json::from_str::<LlmProvider>(raw)
        .ok()
        .unwrap_or_else(|| fallback.clone())
}

pub fn specialist_config_from_storage_model(
    agent: &swarm_agent::Model,
    fallback_provider: &LlmProvider,
) -> SpecialistConfig {
    SpecialistConfig {
        id: Some(agent.id.clone()),
        name: agent.name.clone(),
        agent_type: parse_agent_type(&agent.agent_type, agent.system_prompt.as_deref()),
        llm_provider: parse_llm_provider(&agent.llm_provider, fallback_provider),
        system_prompt_override: agent.system_prompt.clone(),
        max_memory_retrieval: 3,
        capabilities: parse_capabilities(&agent.capabilities),
        enabled: agent.enabled != 0,
    }
}
