//! Lightweight specialist agent

use super::agent_trait::*;
use crate::actions::ActionDef;
use crate::core::llm::{LlmClient, LlmProvider};
use crate::core::orchestra::SubAgentType;
use crate::core::prompt_policy::delegated_policy_v2_block;
use anyhow::Result;
use async_trait::async_trait;

/// Configuration for a specialist agent
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpecialistConfig {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub agent_type: SubAgentType,
    pub llm_provider: LlmProvider,
    #[serde(default)]
    pub system_prompt_override: Option<String>,
    #[serde(default = "default_max_memory")]
    pub max_memory_retrieval: usize,
    #[serde(default)]
    pub capabilities: Vec<AgentCapability>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_max_memory() -> usize {
    3
}
fn default_enabled() -> bool {
    true
}

/// A lightweight specialist agent that wraps an LLM client
pub struct SpecialistAgent {
    id: AgentId,
    config: SpecialistConfig,
    llm: LlmClient,
    available_actions: Vec<ActionDef>,
}

impl SpecialistAgent {
    pub fn new(config: SpecialistConfig, available_actions: Vec<ActionDef>) -> Result<Self> {
        let llm = LlmClient::new(&config.llm_provider)?;
        let id = config
            .id
            .clone()
            .filter(|value| !value.trim().is_empty())
            .map(AgentId)
            .unwrap_or_default();
        Ok(Self {
            id,
            config,
            llm,
            available_actions,
        })
    }

    pub fn config(&self) -> &SpecialistConfig {
        &self.config
    }

    pub fn llm(&self) -> &LlmClient {
        &self.llm
    }

    /// Build the system prompt for this specialist
    fn system_prompt(&self) -> String {
        if let Some(ref override_prompt) = self.config.system_prompt_override {
            return override_prompt.clone();
        }
        self.config.agent_type.system_prompt()
    }

    /// Execute a task using this specialist's LLM
    pub async fn execute_task(&self, task: &str, context: &str) -> Result<String> {
        let system_prompt = format!(
            "{}\n\nYou are part of an agent swarm. Your name is '{}'. \
             Respond with your analysis/result for the delegated task.\n\
             {}\n\n\
             Context from coordinator:\n{}",
            self.system_prompt(),
            self.config.name,
            delegated_policy_v2_block(),
            context
        );

        let supervisor = crate::core::ExecutionSupervisor::default();
        let request = crate::core::ExecutionRequest {
            kind: "swarm_specialist_task".to_string(),
            channel: Some("swarm".to_string()),
            message_preview: Some(task.chars().take(200).collect()),
            ..Default::default()
        };
        let response = crate::core::execution::execute_supervised_transport_chat(
            &supervisor,
            &self.llm,
            &request,
            &system_prompt,
            task,
            &[],
            &self.available_actions,
            Some(60_000),
        )
        .await?;

        Ok(response.content)
    }

    /// Get model name for display
    pub fn model_name(&self) -> String {
        match &self.config.llm_provider {
            LlmProvider::Anthropic { model, .. } => model.clone(),
            LlmProvider::OpenAI { model, .. } => model.clone(),
            LlmProvider::Ollama { model, .. } => model.clone(),
        }
    }
}

#[async_trait]
impl SwarmAgent for SpecialistAgent {
    fn info(&self) -> AgentInfo {
        AgentInfo {
            id: self.id.clone(),
            name: self.config.name.clone(),
            agent_type: format!("{:?}", self.config.agent_type),
            capabilities: self.config.capabilities.clone(),
            status: AgentStatus::Idle,
            llm_model: self.model_name(),
        }
    }

    fn id(&self) -> &AgentId {
        &self.id
    }

    async fn handle_message(&self, message: SwarmMessage) -> Result<SwarmResponse> {
        let start = std::time::Instant::now();
        let context = message.context.unwrap_or_default();
        let content = self.execute_task(&message.content, &context).await?;
        let elapsed = start.elapsed().as_millis() as u64;

        Ok(SwarmResponse {
            agent_id: self.id.clone(),
            content,
            confidence: 0.8,
            tool_calls_made: vec![],
            execution_time_ms: elapsed,
        })
    }

    fn can_handle(&self, task_description: &str) -> f32 {
        let task_lower = task_description.to_lowercase();
        let mut score = 0.0f32;
        let mut total_keywords = 0;

        for cap in &self.config.capabilities {
            for keyword in &cap.keywords {
                total_keywords += 1;
                if task_lower.contains(&keyword.to_lowercase()) {
                    score += 1.0;
                }
            }
        }

        // Also check agent type keywords
        let type_keywords = match self.config.agent_type {
            SubAgentType::Researcher => vec![
                "research",
                "search",
                "find",
                "look up",
                "information",
                "data",
            ],
            SubAgentType::Coder => vec![
                "code",
                "program",
                "debug",
                "script",
                "function",
                "implement",
            ],
            SubAgentType::Analyst => vec![
                "analyze",
                "analysis",
                "data",
                "pattern",
                "trend",
                "statistics",
            ],
            SubAgentType::Writer => vec![
                "write", "draft", "compose", "article", "content", "document",
            ],
            SubAgentType::Validator => {
                vec!["verify", "check", "validate", "review", "test", "audit"]
            }
            SubAgentType::Planner => vec![
                "plan", "schedule", "organize", "strategy", "roadmap", "timeline",
            ],
            SubAgentType::Custom { .. } => vec![],
        };

        for keyword in &type_keywords {
            total_keywords += 1;
            if task_lower.contains(keyword) {
                score += 1.0;
            }
        }

        if total_keywords == 0 {
            return 0.1; // base score for custom agents
        }

        (score / total_keywords as f32).min(1.0)
    }
}
