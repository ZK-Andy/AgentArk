//! Lightweight specialist agent

use super::agent_trait::*;
use crate::actions::ActionDef;
use crate::core::llm::{LlmClient, LlmProvider};
use crate::core::orchestra::SubAgentType;
use crate::core::prompt_policy::delegated_policy_v2_block;
use crate::memory::MemoryEntry;
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

    /// Build the system prompt for this specialist
    fn system_prompt(&self) -> String {
        if let Some(ref override_prompt) = self.config.system_prompt_override {
            return override_prompt.clone();
        }
        self.config.agent_type.system_prompt()
    }

    /// Execute a task using this specialist's LLM with an optional
    /// caller-supplied per-invocation system prompt.
    pub async fn execute_task_with_prompt(
        &self,
        task: &str,
        context: &str,
        system_prompt_override: Option<String>,
    ) -> Result<String> {
        self.execute_task_with_scope_and_prompt(
            task,
            context,
            &[],
            &self.available_actions,
            system_prompt_override,
        )
        .await
    }

    /// Execute a task with task-scoped memories/actions and an optional
    /// caller-supplied system prompt override for this invocation.
    pub async fn execute_task_with_scope_and_prompt(
        &self,
        task: &str,
        context: &str,
        memories: &[MemoryEntry],
        available_actions: &[ActionDef],
        system_prompt_override: Option<String>,
    ) -> Result<String> {
        let system_prompt = format!(
            "{}\n\nYou are part of an agent swarm. Your name is '{}'. \
             Respond with your analysis/result for the delegated task. \
             Stay inside the delegated task packet and use dependency outputs instead of redoing completed work.\n\
             {}\n\n\
             Delegated task packet:\n{}",
            system_prompt_override.unwrap_or_else(|| self.system_prompt()),
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
            memories,
            available_actions,
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
