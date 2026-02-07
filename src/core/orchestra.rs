//! Dynamic Sub-Agent Orchestration Framework (AOrchestra inspired)
//!
//! Enables the main agent to dynamically create specialized sub-agents
//! for complex task decomposition and parallel execution.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::llm::{LlmClient, LlmResponse, ToolCall};
use crate::memory::MemoryEntry;
use crate::actions::ActionDef;

/// Configuration for the orchestration framework
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestraConfig {
    /// Maximum number of sub-agents that can run concurrently
    pub max_concurrent_agents: usize,
    /// Maximum depth of sub-agent delegation
    pub max_delegation_depth: u32,
    /// Timeout for sub-agent execution in seconds
    pub agent_timeout_secs: u64,
    /// Whether sub-agents can create their own sub-agents
    pub allow_nested_delegation: bool,
    /// Default capabilities for all sub-agents
    pub default_capabilities: Vec<String>,
}

impl Default for OrchestraConfig {
    fn default() -> Self {
        Self {
            max_concurrent_agents: 5,
            max_delegation_depth: 3,
            agent_timeout_secs: 60,
            allow_nested_delegation: true,
            default_capabilities: vec![
                "reasoning".to_string(),
                "analysis".to_string(),
            ],
        }
    }
}

/// A specialized sub-agent type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubAgentType {
    /// Research and information gathering
    Researcher,
    /// Code analysis and generation
    Coder,
    /// Data analysis and processing
    Analyst,
    /// Content writing and editing
    Writer,
    /// Fact-checking and validation
    Validator,
    /// Planning and decomposition
    Planner,
    /// Custom agent with specific instructions
    Custom { name: String, instructions: String },
}

impl SubAgentType {
    /// Get the system prompt for this agent type
    pub fn system_prompt(&self) -> String {
        match self {
            Self::Researcher => {
                "You are a Research Agent. Your role is to gather, analyze, and synthesize \
                information from various sources. Focus on accuracy, comprehensiveness, \
                and citing sources when possible. Break down complex research tasks into \
                specific queries."
                    .to_string()
            }
            Self::Coder => {
                "You are a Coding Agent. Your role is to write, analyze, and debug code. \
                Focus on clean, efficient, and well-documented code. Follow best practices \
                and consider edge cases. Explain your implementation decisions."
                    .to_string()
            }
            Self::Analyst => {
                "You are an Analysis Agent. Your role is to examine data, identify patterns, \
                and draw insights. Be thorough in your analysis and present findings clearly. \
                Use quantitative methods when appropriate."
                    .to_string()
            }
            Self::Writer => {
                "You are a Writing Agent. Your role is to create clear, engaging, and \
                well-structured content. Adapt your tone and style to the target audience. \
                Focus on clarity and coherence."
                    .to_string()
            }
            Self::Validator => {
                "You are a Validation Agent. Your role is to verify facts, check logic, \
                and ensure accuracy. Be skeptical and thorough. Flag any inconsistencies \
                or potential errors you find."
                    .to_string()
            }
            Self::Planner => {
                "You are a Planning Agent. Your role is to break down complex tasks into \
                manageable steps, identify dependencies, and create actionable plans. \
                Consider resource constraints and potential risks."
                    .to_string()
            }
            Self::Custom { instructions, .. } => instructions.clone(),
        }
    }

    /// Get the capabilities this agent type has
    #[allow(dead_code)]
    pub fn capabilities(&self) -> Vec<String> {
        match self {
            Self::Researcher => vec![
                "web_search".to_string(),
                "research".to_string(),
                "summarization".to_string(),
            ],
            Self::Coder => vec![
                "code_generation".to_string(),
                "code_analysis".to_string(),
                "debugging".to_string(),
            ],
            Self::Analyst => vec![
                "data_analysis".to_string(),
                "pattern_recognition".to_string(),
                "visualization".to_string(),
            ],
            Self::Writer => vec![
                "content_creation".to_string(),
                "editing".to_string(),
                "formatting".to_string(),
            ],
            Self::Validator => vec![
                "fact_checking".to_string(),
                "logic_verification".to_string(),
                "consistency_check".to_string(),
            ],
            Self::Planner => vec![
                "task_decomposition".to_string(),
                "scheduling".to_string(),
                "resource_planning".to_string(),
            ],
            Self::Custom { .. } => vec!["custom".to_string()],
        }
    }

    /// Get the name of this agent type
    pub fn name(&self) -> String {
        match self {
            Self::Researcher => "Researcher".to_string(),
            Self::Coder => "Coder".to_string(),
            Self::Analyst => "Analyst".to_string(),
            Self::Writer => "Writer".to_string(),
            Self::Validator => "Validator".to_string(),
            Self::Planner => "Planner".to_string(),
            Self::Custom { name, .. } => name.clone(),
        }
    }
}

/// A sub-agent instance
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SubAgent {
    /// Unique identifier
    pub id: Uuid,
    /// Agent type
    pub agent_type: SubAgentType,
    /// Current task
    pub task: String,
    /// Status
    pub status: SubAgentStatus,
    /// Result when completed
    pub result: Option<SubAgentResult>,
    /// Creation time
    pub created_at: DateTime<Utc>,
    /// Parent agent ID (for nested delegation)
    pub parent_id: Option<Uuid>,
    /// Delegation depth
    pub depth: u32,
}

/// Status of a sub-agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SubAgentStatus {
    /// Waiting to be executed
    Pending,
    /// Currently executing
    Running,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed { error: String },
    /// Cancelled
    Cancelled,
}

/// Result from a sub-agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentResult {
    /// The response content
    pub content: String,
    /// Any tool calls made
    pub tool_calls: Vec<ToolCall>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Confidence score
    pub confidence: f32,
    /// Any sub-results from nested agents
    pub sub_results: Vec<SubAgentResult>,
}

/// A task that can be decomposed and delegated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestraTask {
    /// Task identifier
    pub id: Uuid,
    /// Task description
    pub description: String,
    /// Decomposed sub-tasks
    pub sub_tasks: Vec<SubTask>,
    /// Overall status
    pub status: OrchestraTaskStatus,
    /// Final result
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: Uuid,
    pub description: String,
    pub agent_type: SubAgentType,
    pub dependencies: Vec<Uuid>,
    pub status: SubAgentStatus,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrchestraTaskStatus {
    Planning,
    Executing,
    Aggregating,
    Completed,
    Failed { error: String },
}

/// Result of an orchestrated task execution
#[allow(dead_code)]
#[derive(Debug)]
pub struct OrchestraResult {
    /// The final synthesized result
    pub final_result: String,
    /// The orchestrated task with all sub-task details
    pub task: OrchestraTask,
    /// Total execution time in milliseconds
    pub total_time_ms: u64,
}

impl OrchestraResult {
    /// Get the number of sub-agents used
    #[allow(dead_code)]
    pub fn sub_agent_count(&self) -> usize {
        self.task.sub_tasks.len()
    }

    /// Check if all sub-tasks completed successfully
    #[allow(dead_code)]
    pub fn all_successful(&self) -> bool {
        self.task
            .sub_tasks
            .iter()
            .all(|t| matches!(t.status, SubAgentStatus::Completed))
    }
}

/// The main orchestration controller
pub struct Orchestra {
    config: OrchestraConfig,
    active_agents: Arc<RwLock<HashMap<Uuid, SubAgent>>>,
    task_history: Arc<RwLock<Vec<OrchestraTask>>>,
}

impl Orchestra {
    pub fn new(config: OrchestraConfig) -> Self {
        Self {
            config,
            active_agents: Arc::new(RwLock::new(HashMap::new())),
            task_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Decompose a complex task into sub-tasks
    pub async fn decompose_task(
        &self,
        llm: &LlmClient,
        task: &str,
        memories: &[MemoryEntry],
        actions: &[ActionDef],
    ) -> Result<OrchestraTask> {
        // Use the Planner agent to decompose the task
        let planner_prompt = format!(
            "{}\n\n## Task to Decompose\n{}\n\n\
            Analyze this task and break it down into sub-tasks. For each sub-task, specify:\n\
            1. A clear description\n\
            2. The best agent type to handle it (Researcher, Coder, Analyst, Writer, Validator)\n\
            3. Any dependencies on other sub-tasks\n\n\
            Format your response as a list of sub-tasks.",
            SubAgentType::Planner.system_prompt(),
            task
        );

        let response = llm.chat(&planner_prompt, task, memories, actions).await?;

        // Parse the decomposition (simplified - in production, use structured output)
        let sub_tasks = self.parse_decomposition(&response.content, task);

        let orchestra_task = OrchestraTask {
            id: Uuid::new_v4(),
            description: task.to_string(),
            sub_tasks,
            status: OrchestraTaskStatus::Planning,
            result: None,
        };

        // Store in history
        let mut history = self.task_history.write().await;
        history.push(orchestra_task.clone());

        Ok(orchestra_task)
    }

    /// Execute an orchestrated task
    pub async fn execute_task(
        &self,
        llm: Arc<LlmClient>,
        task: &mut OrchestraTask,
        memories: &[MemoryEntry],
        actions: &[ActionDef],
    ) -> Result<String> {
        task.status = OrchestraTaskStatus::Executing;

        // Execute sub-tasks respecting dependencies
        let mut completed: HashMap<Uuid, String> = HashMap::new();
        let mut pending: Vec<&mut SubTask> = task.sub_tasks.iter_mut().collect();

        while !pending.is_empty() {
            // Find tasks with satisfied dependencies
            let ready_indices: Vec<usize> = pending
                .iter()
                .enumerate()
                .filter(|(_, t)| t.dependencies.iter().all(|dep| completed.contains_key(dep)))
                .map(|(i, _)| i)
                .collect();

            if ready_indices.is_empty() && !pending.is_empty() {
                return Err(anyhow!("Circular dependency detected in sub-tasks"));
            }

            // Execute ready tasks (can be parallelized)
            for idx in ready_indices.into_iter().rev() {
                let sub_task = pending.remove(idx);

                // Build context from completed dependencies
                let context: String = sub_task
                    .dependencies
                    .iter()
                    .filter_map(|dep| completed.get(dep))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n\n");

                // Execute sub-task
                let result = self
                    .execute_sub_agent(
                        llm.clone(),
                        &sub_task.agent_type,
                        &sub_task.description,
                        &context,
                        memories,
                        actions,
                        0,
                    )
                    .await?;

                sub_task.status = SubAgentStatus::Completed;
                sub_task.result = Some(result.content.clone());
                completed.insert(sub_task.id, result.content);
            }
        }

        // Aggregate results
        task.status = OrchestraTaskStatus::Aggregating;
        let final_result = self.aggregate_results(llm, task, &completed, memories, actions).await?;

        task.status = OrchestraTaskStatus::Completed;
        task.result = Some(final_result.clone());

        Ok(final_result)
    }

    /// Spawn and execute a single sub-agent
    pub async fn execute_sub_agent(
        &self,
        llm: Arc<LlmClient>,
        agent_type: &SubAgentType,
        task: &str,
        context: &str,
        memories: &[MemoryEntry],
        actions: &[ActionDef],
        depth: u32,
    ) -> Result<SubAgentResult> {
        // Check depth limit
        if depth >= self.config.max_delegation_depth {
            return Err(anyhow!("Maximum delegation depth exceeded"));
        }

        // Check concurrent agent limit
        let agent_count = self.active_agents.read().await.len();
        if agent_count >= self.config.max_concurrent_agents {
            return Err(anyhow!("Maximum concurrent agents exceeded"));
        }

        // Create sub-agent
        let agent = SubAgent {
            id: Uuid::new_v4(),
            agent_type: agent_type.clone(),
            task: task.to_string(),
            status: SubAgentStatus::Running,
            result: None,
            created_at: Utc::now(),
            parent_id: None,
            depth,
        };

        // Register agent
        {
            let mut agents = self.active_agents.write().await;
            agents.insert(agent.id, agent.clone());
        }

        let start_time = std::time::Instant::now();

        // Build prompt for this agent
        let system_prompt = format!(
            "{}\n\n## Context from Previous Steps\n{}",
            agent_type.system_prompt(),
            if context.is_empty() {
                "No previous context.".to_string()
            } else {
                context.to_string()
            }
        );

        // Execute
        let response = llm.chat(&system_prompt, task, memories, actions).await?;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        // Calculate confidence
        let confidence = calculate_response_confidence(&response);

        // Remove from active agents
        {
            let mut agents = self.active_agents.write().await;
            agents.remove(&agent.id);
        }

        Ok(SubAgentResult {
            content: response.content,
            tool_calls: response.tool_calls,
            execution_time_ms,
            confidence,
            sub_results: vec![],
        })
    }

    /// Spawn a sub-agent for a specific type of task
    #[allow(dead_code)]
    pub async fn spawn_agent(
        &self,
        llm: Arc<LlmClient>,
        agent_type: SubAgentType,
        task: &str,
        memories: &[MemoryEntry],
        actions: &[ActionDef],
    ) -> Result<SubAgentResult> {
        self.execute_sub_agent(llm, &agent_type, task, "", memories, actions, 0)
            .await
    }

    /// Auto-detect the best agent type for a task
    pub fn detect_agent_type(&self, task: &str) -> SubAgentType {
        let task_lower = task.to_lowercase();

        // Simple keyword-based detection
        if task_lower.contains("search")
            || task_lower.contains("find")
            || task_lower.contains("research")
            || task_lower.contains("look up")
        {
            return SubAgentType::Researcher;
        }

        if task_lower.contains("code")
            || task_lower.contains("implement")
            || task_lower.contains("function")
            || task_lower.contains("debug")
            || task_lower.contains("program")
        {
            return SubAgentType::Coder;
        }

        if task_lower.contains("analyze")
            || task_lower.contains("data")
            || task_lower.contains("statistics")
            || task_lower.contains("pattern")
        {
            return SubAgentType::Analyst;
        }

        if task_lower.contains("write")
            || task_lower.contains("draft")
            || task_lower.contains("compose")
            || task_lower.contains("edit")
        {
            return SubAgentType::Writer;
        }

        if task_lower.contains("verify")
            || task_lower.contains("check")
            || task_lower.contains("validate")
            || task_lower.contains("confirm")
        {
            return SubAgentType::Validator;
        }

        if task_lower.contains("plan")
            || task_lower.contains("organize")
            || task_lower.contains("schedule")
            || task_lower.contains("steps")
        {
            return SubAgentType::Planner;
        }

        // Default to Planner for complex tasks
        SubAgentType::Planner
    }

    /// Parse decomposition response into sub-tasks
    fn parse_decomposition(&self, response: &str, original_task: &str) -> Vec<SubTask> {
        let mut sub_tasks = Vec::new();

        // Simple line-based parsing (in production, use structured output)
        for line in response.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Try to extract task description
            let description = line
                .trim_start_matches(|c: char| c.is_numeric() || c == '.' || c == '-' || c == '*')
                .trim();

            if description.len() > 10 {
                let agent_type = self.detect_agent_type(description);

                sub_tasks.push(SubTask {
                    id: Uuid::new_v4(),
                    description: description.to_string(),
                    agent_type,
                    dependencies: vec![],
                    status: SubAgentStatus::Pending,
                    result: None,
                });
            }
        }

        // If parsing failed, create a single task
        if sub_tasks.is_empty() {
            sub_tasks.push(SubTask {
                id: Uuid::new_v4(),
                description: original_task.to_string(),
                agent_type: SubAgentType::Planner,
                dependencies: vec![],
                status: SubAgentStatus::Pending,
                result: None,
            });
        }

        // Set up simple sequential dependencies
        let ids: Vec<Uuid> = sub_tasks.iter().map(|t| t.id).collect();
        for i in 1..sub_tasks.len() {
            sub_tasks[i].dependencies.push(ids[i - 1]);
        }

        sub_tasks
    }

    /// Aggregate results from all sub-tasks
    async fn aggregate_results(
        &self,
        llm: Arc<LlmClient>,
        task: &OrchestraTask,
        results: &HashMap<Uuid, String>,
        memories: &[MemoryEntry],
        actions: &[ActionDef],
    ) -> Result<String> {
        // If only one sub-task, return its result directly
        if results.len() == 1 {
            return Ok(results.values().next().unwrap().clone());
        }

        // Build aggregation prompt
        let results_text: String = task
            .sub_tasks
            .iter()
            .filter_map(|st| {
                results.get(&st.id).map(|r| {
                    format!("## {} ({})\n{}", st.description, st.agent_type.name(), r)
                })
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        let aggregation_prompt = format!(
            "You are synthesizing results from multiple specialized agents.\n\n\
            ## Original Task\n{}\n\n\
            ## Results from Sub-Agents\n{}\n\n\
            Synthesize these results into a coherent, comprehensive response to the original task.",
            task.description, results_text
        );

        let response = llm
            .chat(&aggregation_prompt, &task.description, memories, actions)
            .await?;

        Ok(response.content)
    }

    /// Get status of all active agents
    #[allow(dead_code)]
    pub async fn get_active_agents(&self) -> Vec<SubAgent> {
        let agents = self.active_agents.read().await;
        agents.values().cloned().collect()
    }

    /// Get task history
    #[allow(dead_code)]
    pub async fn get_task_history(&self) -> Vec<OrchestraTask> {
        let history = self.task_history.read().await;
        history.clone()
    }

    /// Auto-orchestrate a task with full context
    #[allow(dead_code)]
    pub async fn auto_orchestrate(
        &self,
        llm: Arc<LlmClient>,
        task: &str,
        _system_prompt: &str,
        memories: &[MemoryEntry],
        actions: &[ActionDef],
    ) -> Result<OrchestraResult> {
        let start_time = std::time::Instant::now();

        // Decompose the task
        let mut orchestra_task = self.decompose_task(&llm, task, memories, actions).await?;

        tracing::info!(
            "Task decomposed into {} sub-tasks",
            orchestra_task.sub_tasks.len()
        );

        // Execute
        let final_result = self
            .execute_task(llm, &mut orchestra_task, memories, actions)
            .await?;

        let total_time_ms = start_time.elapsed().as_millis() as u64;

        Ok(OrchestraResult {
            final_result,
            task: orchestra_task,
            total_time_ms,
        })
    }

    /// Cancel an active agent
    #[allow(dead_code)]
    pub async fn cancel_agent(&self, agent_id: Uuid) -> Result<()> {
        let mut agents = self.active_agents.write().await;
        if let Some(agent) = agents.get_mut(&agent_id) {
            agent.status = SubAgentStatus::Cancelled;
            agents.remove(&agent_id);
            Ok(())
        } else {
            Err(anyhow!("Agent not found: {}", agent_id))
        }
    }
}

/// Calculate confidence score for a response
fn calculate_response_confidence(response: &LlmResponse) -> f32 {
    let mut confidence = 0.6; // Base confidence

    let word_count = response.content.split_whitespace().count();
    if word_count > 30 {
        confidence += 0.1;
    }
    if word_count > 100 {
        confidence += 0.1;
    }

    if !response.tool_calls.is_empty() {
        confidence += 0.1;
    }

    // Penalize uncertainty markers
    let content_lower = response.content.to_lowercase();
    let uncertainty_count = ["not sure", "might", "possibly", "maybe", "unclear"]
        .iter()
        .filter(|m| content_lower.contains(*m))
        .count();
    confidence -= uncertainty_count as f32 * 0.05;

    confidence.clamp(0.0, 1.0)
}

/// Quick function to orchestrate a task automatically
#[allow(dead_code)]
pub async fn auto_orchestrate(
    llm: Arc<LlmClient>,
    task: &str,
    memories: &[MemoryEntry],
    actions: &[ActionDef],
) -> Result<String> {
    let orchestra = Orchestra::new(OrchestraConfig::default());

    // Decompose the task
    let mut orchestra_task = orchestra.decompose_task(&llm, task, memories, actions).await?;

    // Execute
    orchestra
        .execute_task(llm, &mut orchestra_task, memories, actions)
        .await
}
