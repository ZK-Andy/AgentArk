//! Core Agent implementation

use crate::{
    identity::IdentityManager,
    memory::CognitiveMemory,
    proofs::ProofEngine,
    runtime::ActionRuntime,
    safety::SafetyEngine,
    security::SecurityGuard,
    storage::Storage,
};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{
    llm::LlmClient,
    task::TaskQueue,
    AgentConfig,
    parallel::{ParallelThinkingController, ParallelConfig},
    orchestra::{Orchestra, OrchestraConfig},
};

/// Safe string truncation that respects UTF-8 character boundaries
fn safe_truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    }
}

/// Query complexity classification
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueryComplexity {
    /// Simple query - direct response
    Simple,
    /// Medium complexity - use parallel thinking
    Medium,
    /// Complex multi-step task - use orchestra
    Complex,
}

/// Conversation message for history tracking
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// The main Agent struct - orchestrates all subsystems
pub struct Agent {
    /// Persistent storage
    pub storage: Storage,

    /// Decentralized identity manager
    pub identity: IdentityManager,

    /// Cognitive memory system (episodic, semantic, procedural)
    pub memory: CognitiveMemory,

    /// Safety policy engine
    pub safety: SafetyEngine,

    /// Execution proof generator
    pub proofs: ProofEngine,

    /// Action runtime (WASM + Docker sandbox)
    pub runtime: ActionRuntime,

    /// LLM client for reasoning
    pub llm: LlmClient,

    /// Task queue for autonomous execution
    pub tasks: Arc<RwLock<TaskQueue>>,

    /// Configuration
    pub config: AgentConfig,

    /// Config directory path
    pub config_dir: PathBuf,

    /// Parallel thinking controller for improved reasoning
    pub parallel_controller: ParallelThinkingController,

    /// Orchestra for sub-agent delegation
    pub orchestra: Orchestra,

    /// Security guard for prompt injection/leakage protection
    pub security: SecurityGuard,

    /// Conversation history per channel (keeps last N messages)
    pub conversation_history: Arc<RwLock<std::collections::HashMap<String, Vec<ConversationMessage>>>>,

    /// User profile (name, location, preferences) learned during onboarding
    pub user_profile: Arc<RwLock<UserProfile>>,

    /// Last execution trace - shows what the agent actually did
    pub last_trace: Arc<RwLock<ExecutionTrace>>,

    /// Trace history - stores last 100 execution traces
    pub trace_history: Arc<RwLock<Vec<ExecutionTrace>>>,

    /// External service integrations (Calendar, WhatsApp, etc.)
    pub integrations: crate::integrations::IntegrationManager,
}

/// User profile collected during onboarding
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct UserProfile {
    pub name: Option<String>,
    pub location: Option<String>,
    pub timezone: Option<String>,
    pub language: Option<String>,
    pub tone: Option<String>,
    pub email_format: Option<String>,
    pub preferences: Option<String>,
    pub onboarding_complete: bool,
}

/// Execution trace step - records what the agent actually did
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionStep {
    pub icon: String,
    pub title: String,
    pub detail: String,
    pub step_type: String,  // info, success, thinking, warning
    pub data: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub duration_ms: Option<u64>,
}

/// Full execution trace for a message
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ExecutionTrace {
    /// Unique ID for this trace
    pub id: String,
    pub message: String,
    pub channel: String,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub steps: Vec<ExecutionStep>,
    pub proof_id: Option<String>,
    /// Response/result of the execution
    pub response: Option<String>,
}

impl Agent {
    /// Initialize the agent with all subsystems
    pub async fn init(config_dir: &Path, data_dir: &Path) -> Result<Self> {
        // Initialize storage
        let storage = Storage::new(data_dir).await?;

        // Initialize identity system
        let identity = IdentityManager::load_or_create(data_dir).await?;

        // Initialize memory system
        let memory = CognitiveMemory::new(data_dir, storage.clone()).await?;

        // Initialize safety engine
        let safety = SafetyEngine::new(config_dir)?;

        // Initialize proof system
        let proofs = ProofEngine::new(data_dir, identity.signing_key())?;

        // Initialize action runtime
        let mut runtime = ActionRuntime::new(config_dir, data_dir).await?;

        // Load configuration
        let config = AgentConfig::load(config_dir)?;

        // Initialize LLM client
        let llm = LlmClient::new(&config.llm)?;

        // Initialize task queue
        let tasks = Arc::new(RwLock::new(TaskQueue::new()));

        // Wire task queue into runtime so list_tasks action can access it
        runtime.set_task_queue(tasks.clone());

        // Initialize parallel thinking controller
        let parallel_controller = ParallelThinkingController::new(ParallelConfig::default());

        // Initialize orchestra for sub-agent delegation
        let orchestra = Orchestra::new(OrchestraConfig::default());

        // Initialize security guard for prompt injection/leakage protection
        let security = SecurityGuard::new(true); // Strict mode enabled

        // Load persisted user profile (if any)
        let user_profile = match storage.get("user_profile").await {
            Ok(Some(bytes)) => serde_json::from_slice::<UserProfile>(&bytes)
                .unwrap_or_default(),
            _ => UserProfile::default(),
        };

        // Load persisted tasks (if any)
        if let Ok(stored_tasks) = storage.get_tasks().await {
            let mut queue = tasks.write().await;
            for t in stored_tasks {
                let id = uuid::Uuid::parse_str(&t.id).unwrap_or_else(|_| uuid::Uuid::new_v4());
                let arguments = serde_json::from_str(&t.arguments).unwrap_or_else(|_| serde_json::json!({}));
                let approval = serde_json::from_str(&t.approval).unwrap_or(super::task::TaskApproval::Auto);
                let status = serde_json::from_str(&t.status).unwrap_or(super::task::TaskStatus::Pending);
                let created_at = chrono::DateTime::parse_from_rfc3339(&t.created_at)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());
                let scheduled_for = t.scheduled_for
                    .as_deref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.with_timezone(&chrono::Utc));
                let proof_id = t.proof_id
                    .as_deref()
                    .and_then(|s| uuid::Uuid::parse_str(s).ok());

                queue.add(super::task::Task {
                    id,
                    description: t.description,
                    action: t.action,
                    arguments,
                    approval,
                    capabilities: vec![],
                    status,
                    created_at,
                    scheduled_for,
                    cron: t.cron,
                    result: t.result,
                    proof_id,
                });
            }
        }

        // Initialize integration manager
        let integrations = crate::integrations::IntegrationManager::new(config_dir);

        Ok(Self {
            storage,
            identity,
            memory,
            safety,
            proofs,
            runtime,
            llm,
            tasks,
            config,
            config_dir: config_dir.to_path_buf(),
            parallel_controller,
            orchestra,
            security,
            conversation_history: Arc::new(RwLock::new(std::collections::HashMap::new())),
            user_profile: Arc::new(RwLock::new(user_profile)),
            last_trace: Arc::new(RwLock::new(ExecutionTrace::default())),
            trace_history: Arc::new(RwLock::new(Vec::new())),
            integrations,
        })
    }

    /// Process an incoming message and generate a response
    pub async fn process_message(&mut self, message: &str, channel: &str) -> Result<String> {
        let start_time = chrono::Utc::now();
        tracing::info!("Processing message from {}: {}", channel, message);

        // Security check: Detect prompt injection and leakage attempts
        let sanitized = self.security.sanitize_input(message);
        if !sanitized.is_safe {
            if let Some(ref injection_type) = sanitized.injection_type {
                tracing::warn!("Security: Detected {:?} attempt from {}", injection_type, channel);
                return Ok(crate::security::get_safe_response(injection_type).to_string());
            }
        }
        let message = &sanitized.text; // Use sanitized input

        // Initialize execution trace
        let trace_id = uuid::Uuid::new_v4().to_string();
        {
            let mut trace = self.last_trace.write().await;
            *trace = ExecutionTrace {
                id: trace_id.clone(),
                message: message.to_string(),
                channel: channel.to_string(),
                started_at: Some(start_time),
                completed_at: None,
                steps: vec![],
                proof_id: None,
                response: None,
            };
            trace.steps.push(ExecutionStep {
                icon: "📩".to_string(),
                title: "Message Received".to_string(),
                detail: format!("Channel: {} | Length: {} chars", channel, message.len()),
                step_type: "info".to_string(),
                data: Some(safe_truncate(message, 100)),
                timestamp: start_time,
                duration_ms: None,
            });
        }

        // 0. Check for onboarding response pattern and extract profile
        self.try_extract_profile(message).await;

        // 1. Store in episodic memory
        self.memory
            .add_episode(
                message.to_string(),
                crate::memory::EpisodeContext {
                    channel: channel.to_string(),
                    timestamp: chrono::Utc::now(),
                    location: None,
                    participants: vec![],
                },
            )
            .await?;

        {
            let mut trace = self.last_trace.write().await;
            let stored_preview = safe_truncate(message, 100);
            trace.steps.push(ExecutionStep {
                icon: "🧬".to_string(),
                title: "Stored in Episodic Memory".to_string(),
                detail: format!("Total memories: {} | Channel: {}", self.memory.entry_count(), channel),
                step_type: "success".to_string(),
                data: Some(format!("📝 Stored: \"{}\"", stored_preview)),
                timestamp: chrono::Utc::now(),
                duration_ms: None,
            });
        }

        // 2. Add to conversation history
        {
            let mut history = self.conversation_history.write().await;
            let channel_history = history.entry(channel.to_string()).or_insert_with(Vec::new);
            channel_history.push(ConversationMessage {
                role: "user".to_string(),
                content: message.to_string(),
                timestamp: chrono::Utc::now(),
            });
            // Keep only last 10 messages per channel (cost optimization)
            if channel_history.len() > 10 {
                channel_history.drain(0..channel_history.len() - 10);
            }
        }

        // 3. Retrieve relevant memories (limit to 3 for cost efficiency)
        let memory_start = std::time::Instant::now();
        let relevant_memories = self.memory.retrieve_relevant(message, 3).await?;
        let memory_duration = memory_start.elapsed().as_millis() as u64;

        {
            let mut trace = self.last_trace.write().await;
            trace.steps.push(ExecutionStep {
                icon: "🔍".to_string(),
                title: "Memory Retrieval".to_string(),
                detail: format!("Found {} relevant memories (searched for: \"{}\")",
                    relevant_memories.len(),
                    safe_truncate(message, 30)
                ),
                step_type: "success".to_string(),
                data: if relevant_memories.is_empty() {
                    Some("No relevant memories found".to_string())
                } else {
                    // Show all retrieved memories with more detail
                    Some(relevant_memories.iter().enumerate().map(|(i, m)| {
                        let preview = safe_truncate(&m.content, 150);
                        let timestamp = m.timestamp.format("%Y-%m-%d %H:%M").to_string();
                        format!("{}. [{}] {}", i + 1, timestamp, preview)
                    }).collect::<Vec<_>>().join("\n\n"))
                },
                timestamp: chrono::Utc::now(),
                duration_ms: Some(memory_duration),
            });
        }

        // 4. Build context for LLM
        let system_prompt = self.build_system_prompt(&relevant_memories).await?;

        // 5. Get conversation history for context (optimized for cost)
        let conversation_history: Vec<ConversationMessage> = {
            let history = self.conversation_history.read().await;
            let full_history = history.get(channel).cloned().unwrap_or_default();
            // Only send last 5 messages to LLM (cost optimization)
            // Also truncate very long messages
            let mut recent: Vec<_> = full_history.into_iter()
                .rev()
                .take(5)
                .map(|mut m| {
                    if m.content.chars().count() > 500 {
                        m.content = safe_truncate(&m.content, 500);
                    }
                    m
                })
                .collect();
            recent.reverse();
            recent
        };

        // 6. Get available actions (filtered for relevance to reduce token usage)
        let all_actions = self.runtime.list_actions().await?;
        let msg_lower = message.to_lowercase();

        // Build context from recent conversation history to detect ongoing topics
        // This prevents tools from disappearing on follow-up messages like "more" or "older"
        let recent_context: String = {
            let history = self.conversation_history.read().await;
            history.get(channel)
                .map(|msgs| {
                    msgs.iter()
                        .rev()
                        .take(4) // last 4 messages (2 turns)
                        .map(|m| m.content.to_lowercase())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default()
        };

        // Filter actions based on keyword matching with query + recent context
        // This significantly reduces tokens sent to LLM
        let available_actions: Vec<_> = all_actions.into_iter()
            .filter(|action| {
                // Always include core actions
                let core_actions = ["web_search", "http_get", "file_read", "file_write"];
                if core_actions.contains(&action.name.as_str()) {
                    return true;
                }

                // Include list_tasks/schedule_task when current or recent messages mention relevant terms
                if action.name == "list_tasks" || action.name == "schedule_task" {
                    let task_keywords = ["task", "goal", "pending", "schedule", "routine", "agenda", "plan", "todo", "remind", "daily", "recurring", "cron"];
                    return task_keywords.iter().any(|kw| msg_lower.contains(kw) || recent_context.contains(kw));
                }

                // Include gmail actions when current or recent messages mention email-related terms
                if action.name == "gmail_scan" || action.name == "gmail_reply" {
                    let gmail_keywords = ["gmail", "email", "mail", "inbox", "unread", "message", "send", "reply", "meeting", "interview", "calendar", "invite", "receipt", "scan"];
                    return gmail_keywords.iter().any(|kw| msg_lower.contains(kw) || recent_context.contains(kw));
                }

                // Include if action name or description matches query keywords
                let action_text = format!("{} {}", action.name, action.description).to_lowercase();
                let keywords: Vec<&str> = msg_lower.split_whitespace()
                    .filter(|w| w.len() > 3)
                    .collect();

                keywords.iter().any(|kw| action_text.contains(kw))
                    || action.name.to_lowercase().split('-').any(|part| msg_lower.contains(part))
            })
            .take(10) // Max 10 actions to reduce tokens
            .collect();

        // 7. Classify query complexity and route appropriately
        let complexity = self.classify_complexity(message);
        tracing::debug!("Query complexity: {:?}", complexity);

        {
            let mut trace = self.last_trace.write().await;
            let (complexity_str, complexity_desc) = match complexity {
                QueryComplexity::Simple => ("Simple", "Direct LLM response"),
                QueryComplexity::Medium => ("Medium", "Using Parallel Thinking"),
                QueryComplexity::Complex => ("Complex", "Using Orchestra delegation"),
            };
            trace.steps.push(ExecutionStep {
                icon: "🎯".to_string(),
                title: "Query Complexity Classification".to_string(),
                detail: format!("{} → {}", complexity_str, complexity_desc),
                step_type: "thinking".to_string(),
                data: Some(format!("Actions available: {}", available_actions.len())),
                timestamp: chrono::Utc::now(),
                duration_ms: None,
            });
        }

        let llm_start = std::time::Instant::now();
        let (response, tool_calls) = match complexity {
            QueryComplexity::Complex => {
                // Use orchestra for complex multi-step tasks
                tracing::info!("Using Orchestra for complex task delegation");
                {
                    let mut trace = self.last_trace.write().await;
                    trace.steps.push(ExecutionStep {
                        icon: "🎭".to_string(),
                        title: "Orchestra Delegation Started".to_string(),
                        detail: "Spawning sub-agents for complex task".to_string(),
                        step_type: "thinking".to_string(),
                        data: None,
                        timestamp: chrono::Utc::now(),
                        duration_ms: None,
                    });
                }
                let llm_arc = Arc::new(self.llm.clone());
                let result = self.orchestra
                    .auto_orchestrate(
                        llm_arc,
                        message,
                        &system_prompt,
                        &relevant_memories,
                        &available_actions,
                    )
                    .await?;
                (result.final_result, vec![])
            }
            QueryComplexity::Medium => {
                // Use parallel thinking for medium complexity
                tracing::info!("Using Parallel Thinking for improved reasoning");
                {
                    let mut trace = self.last_trace.write().await;
                    trace.steps.push(ExecutionStep {
                        icon: "🔀".to_string(),
                        title: "Parallel Thinking Started".to_string(),
                        detail: "Exploring multiple reasoning paths".to_string(),
                        step_type: "thinking".to_string(),
                        data: None,
                        timestamp: chrono::Utc::now(),
                        duration_ms: None,
                    });
                }
                let llm_arc = Arc::new(self.llm.clone());
                let result = self.parallel_controller
                    .think_with_llm(
                        llm_arc,
                        &system_prompt,
                        message,
                        &relevant_memories,
                        &available_actions,
                    )
                    .await?;

                {
                    let mut trace = self.last_trace.write().await;
                    trace.steps.push(ExecutionStep {
                        icon: "✅".to_string(),
                        title: "Parallel Thinking Complete".to_string(),
                        detail: format!("{} paths explored, {:.1}% cost savings",
                            result.path_results.len(), result.cost_savings_percent()),
                        step_type: "success".to_string(),
                        data: Some(format!("Confidence: {:.2}", result.confidence())),
                        timestamp: chrono::Utc::now(),
                        duration_ms: None,
                    });
                }

                tracing::info!(
                    "Parallel thinking complete: {} paths, {:.1}% cost savings, confidence: {:.2}",
                    result.path_results.len(),
                    result.cost_savings_percent(),
                    result.confidence()
                );

                (result.final_response.content.clone(), result.final_response.tool_calls.clone())
            }
            QueryComplexity::Simple => {
                // Direct LLM call for simple queries - include conversation history
                {
                    let mut trace = self.last_trace.write().await;
                    trace.steps.push(ExecutionStep {
                        icon: "🤖".to_string(),
                        title: "LLM Request".to_string(),
                        detail: "Direct query to language model".to_string(),
                        step_type: "thinking".to_string(),
                        data: None,
                        timestamp: chrono::Utc::now(),
                        duration_ms: None,
                    });
                }
                let llm_response = self
                    .llm
                    .chat_with_history(
                        &system_prompt,
                        message,
                        &conversation_history,
                        &relevant_memories,
                        &available_actions,
                    )
                    .await?;
                (llm_response.content.clone(), llm_response.tool_calls.clone())
            }
        };
        let llm_duration = llm_start.elapsed().as_millis() as u64;

        {
            let mut trace = self.last_trace.write().await;
            trace.steps.push(ExecutionStep {
                icon: "💬".to_string(),
                title: "LLM Response Received".to_string(),
                detail: format!("Response length: {} chars | Tool calls: {}", response.len(), tool_calls.len()),
                step_type: "success".to_string(),
                data: None,
                timestamp: chrono::Utc::now(),
                duration_ms: Some(llm_duration),
            });
        }

        // 6. Execute any tool calls
        let llm_response = super::llm::LlmResponse {
            content: response,
            tool_calls: tool_calls.clone(),
        };

        if !tool_calls.is_empty() {
            let mut trace = self.last_trace.write().await;
            for call in &tool_calls {
                trace.steps.push(ExecutionStep {
                    icon: "⚡".to_string(),
                    title: format!("Executing Action: {}", call.name),
                    detail: "Running in sandboxed environment".to_string(),
                    step_type: "thinking".to_string(),
                    data: Some(format!("Args: {}", serde_json::to_string(&call.arguments).unwrap_or_default())),
                    timestamp: chrono::Utc::now(),
                    duration_ms: None,
                });
            }
        }

        let response = self.execute_tool_calls(&llm_response).await?;

        // 7. Generate execution proof
        let proof = self.proofs.generate_proof(
            message,
            &response,
            &llm_response.tool_calls,
        )?;
        tracing::debug!("Execution proof: {}", proof.id);

        {
            let mut trace = self.last_trace.write().await;
            trace.steps.push(ExecutionStep {
                icon: "🔐".to_string(),
                title: "Execution Proof Generated".to_string(),
                detail: format!("Proof ID: {}", proof.id),
                step_type: "success".to_string(),
                data: Some(format!("Signed with DID: {}...", &self.identity.did()[..30.min(self.identity.did().len())])),
                timestamp: chrono::Utc::now(),
                duration_ms: None,
            });
            trace.proof_id = Some(proof.id.to_string());
        }

        // 9. Store response in episodic memory
        self.memory
            .add_episode(
                response.clone(),
                crate::memory::EpisodeContext {
                    channel: "agent".to_string(),
                    timestamp: chrono::Utc::now(),
                    location: None,
                    participants: vec![],
                },
            )
            .await?;

        // 10. Add assistant response to conversation history
        {
            let mut history = self.conversation_history.write().await;
            if let Some(channel_history) = history.get_mut(channel) {
                channel_history.push(ConversationMessage {
                    role: "assistant".to_string(),
                    content: response.clone(),
                    timestamp: chrono::Utc::now(),
                });
            }
        }

        // Finalize trace and add to history
        {
            let mut trace = self.last_trace.write().await;
            let end_time = chrono::Utc::now();
            let total_duration = if let Some(start) = trace.started_at {
                (end_time - start).num_milliseconds() as u64
            } else {
                0
            };
            trace.completed_at = Some(end_time);
            trace.response = Some(response.clone());
            trace.steps.push(ExecutionStep {
                icon: "✅".to_string(),
                title: "Response Complete".to_string(),
                detail: format!("Total time: {}ms | Response: {} chars", total_duration, response.len()),
                step_type: "success".to_string(),
                data: None,
                timestamp: end_time,
                duration_ms: Some(total_duration),
            });

            // Add to trace history (keep last 100)
            let mut history = self.trace_history.write().await;
            history.insert(0, trace.clone()); // Add to front
            if history.len() > 100 {
                history.truncate(100); // Keep only last 100
            }
        }

        // Security: Filter output to prevent sensitive data leakage
        let filtered = self.security.filter_output(&response);
        if !filtered.redactions.is_empty() {
            tracing::warn!("Security: Redacted sensitive data from output: {:?}", filtered.redactions);
        }

        Ok(filtered.text)
    }

    /// Try to extract user profile from onboarding-style responses
    async fn try_extract_profile(&self, message: &str) {
        // Check if this looks like an onboarding response (short, comma-separated)
        let parts: Vec<&str> = message.split(',').map(|s| s.trim()).collect();

        if parts.len() >= 2 && parts.len() <= 4 {
            // Looks like "name, location, interest" pattern
            let mut profile = self.user_profile.write().await;

            // Only extract if we haven't completed onboarding
            if !profile.onboarding_complete {
                if let Some(name) = parts.get(0) {
                    if !name.is_empty() && name.len() < 50 {
                        profile.name = Some(name.to_string());
                    }
                }
                if let Some(location) = parts.get(1) {
                    if !location.is_empty() && location.len() < 100 {
                        profile.location = Some(location.to_string());
                    }
                }
                if let Some(pref) = parts.get(2) {
                    if !pref.is_empty() {
                        profile.preferences = Some(pref.to_string());
                    }
                }

                // Mark onboarding as complete if we got at least name
                if profile.name.is_some() {
                    profile.onboarding_complete = true;
                    tracing::info!("User profile extracted: {:?}", *profile);

                    if let Ok(bytes) = serde_json::to_vec(&*profile) {
                        if let Err(e) = self.storage.set("user_profile", &bytes).await {
                            tracing::warn!("Failed to persist user profile: {}", e);
                        }
                    }
                }
            }
        }
    }

    /// Classify query complexity for routing
    fn classify_complexity(&self, message: &str) -> QueryComplexity {
        let msg_lower = message.to_lowercase();
        let word_count = message.split_whitespace().count();

        // Complex task indicators - multi-step, research, creation
        let complex_indicators = [
            "research", "investigate", "analyze and", "create a", "build a",
            "develop", "implement", "design", "plan", "compare and",
            "write a report", "write an article", "comprehensive",
            "step by step", "multiple", "all of", "each of",
        ];

        // Medium complexity indicators - reasoning, analysis
        let medium_indicators = [
            "explain", "why", "how does", "what is the difference",
            "should i", "which is better", "pros and cons",
            "analyze", "evaluate", "recommend", "suggest",
            "help me understand", "clarify",
        ];

        // Check for complex patterns
        for indicator in &complex_indicators {
            if msg_lower.contains(indicator) {
                return QueryComplexity::Complex;
            }
        }

        // Long messages with questions are often complex
        if word_count > 50 && msg_lower.contains('?') {
            return QueryComplexity::Complex;
        }

        // Check for medium complexity patterns
        for indicator in &medium_indicators {
            if msg_lower.contains(indicator) {
                return QueryComplexity::Medium;
            }
        }

        // Messages with multiple sentences might need more thought
        let sentence_count = message.matches('.').count() + message.matches('?').count();
        if sentence_count >= 3 || word_count > 30 {
            return QueryComplexity::Medium;
        }

        QueryComplexity::Simple
    }

    /// Build system prompt with relevant context
    async fn build_system_prompt(
        &self,
        memories: &[crate::memory::MemoryEntry],
    ) -> Result<String> {
        let user_profile = self.user_profile.read().await;
        let bot_name = &self.config.name;
        let personality = &self.config.personality;

        // Map personality to behavioral traits (Big Five grounded)
        let (style_desc, tone_examples) = match personality.as_str() {
            "professional" => (
                "Communicate precisely and respectfully. Structured thinking, measured tone. Like a trusted senior colleague.",
                "Example tone: 'Here's what I found...' / 'Based on the data, I'd suggest...' / 'Let me look into that.'"
            ),
            "casual" => (
                "Keep it relaxed and conversational. Talk like a friend who happens to be really knowledgeable. Use natural, everyday language.",
                "Example tone: 'Oh nice, let me check...' / 'So basically...' / 'Yeah that makes sense, here's the deal...'"
            ),
            "technical" => (
                "Be thorough and precise when explaining technical concepts, but still approachable. Think senior engineer explaining to a peer.",
                "Example tone: 'The issue here is...' / 'Under the hood, what's happening is...' / 'Here's the breakdown...'"
            ),
            "creative" => (
                "Be expressive and imaginative. Use vivid analogies, make connections others wouldn't. Think curious polymath.",
                "Example tone: 'That's interesting because...' / 'Think of it like...' / 'Here's a different angle...'"
            ),
            "concise" => (
                "Get to the point fast. No filler. Every word earns its place.",
                "Example tone: 'Done.' / 'Three options: ...' / 'Short answer: X. Want details?'"
            ),
            _ => (
                "Be warm but not syrupy. Genuinely helpful, like a sharp friend who pays attention and remembers things. Natural, not performative.",
                "Example tone: 'Hey, sure thing...' / 'Got it, let me...' / 'Ah yeah, I remember you mentioned...'"
            ),
        };

        let mut prompt = format!(
            r#"You are {bot_name}.

## Who You Are
You're not a generic assistant — you have a personality. You're sharp, attentive, and genuinely useful. You remember things, you pick up on context, and you talk like a real person, not a customer service bot.

{style_desc}
{tone_examples}

## How You Talk (Conversational Maxims)
- **Be natural**: Talk the way a thoughtful human would. No corporate-speak, no filler phrases, no "Great question!" openers.
- **Match the energy**: Short question? Short answer. Deep question? Thoughtful response. "Hey" deserves "Hey, what's up?" not a paragraph.
- **Don't parrot information back**: If you know the user's name, just use it naturally once in a while — NEVER say "Hello [Name] from [City]" or recite their profile. A friend doesn't greet you by listing your bio.
- **Be honest**: When you don't know something, say so. "Not sure about that" beats a confident wrong answer.
- **Show, don't tell**: Don't describe your personality — just embody it. Never say "As an AI..." or "I'm designed to..."
- **Stay brief by default**: Expand only when the topic warrants it or the user asks for detail.
- **Read the room**: If someone's frustrated, acknowledge it. If they're excited, match it. If they just want a quick answer, don't lecture.

## What You Can Do
- Run actions and tools (sandboxed execution)
- Schedule tasks (one-time or recurring via schedule_task)
- Query tasks, goals, and routines (via list_tasks) — use this when asked about pending items, agenda, goals, or scheduled things
- **Read and manage Gmail** (via gmail_scan and gmail_reply) — scan inbox, search emails, send replies
- Remember past conversations and learn preferences over time
- Help with coding, research, analysis, and everyday tasks
- Push results to Telegram automatically

## Gmail Intelligence
When scanning emails, DON'T just dump raw data. Be smart about it:
- **Classify**: Separate into categories — Important (from real people, action needed), Newsletters, Receipts/Orders, Notifications, Spam
- **Highlight**: Flag upcoming meetings, interviews, events, deadlines, or anything time-sensitive
- **Summarize**: Show sender, subject, and a one-line gist — not raw headers
- **Format nicely**: Use clear sections with headers, not a wall of text
- When asked "can you access my gmail?" or similar — confirm yes and ask what they'd like: scan inbox, search for something specific, check for meetings, etc. Don't immediately dump all emails.
- Example good response to "check my email":
  "You have 3 new emails. Here's the rundown:
   **Action needed:**
   - Meeting invite from Sarah for tomorrow 3pm — Project Review
   **FYI:**
   - Security alert from Google (new sign-in detected)
   **Newsletters:**
   - Unstract webinar invite (Feb 4)"

## Action Principles
- EXECUTE FIRST: When asked to do something, just do it. Don't ask for confirmation on obvious tasks.
- USE DEFAULTS: If an action has saved parameters, use them. Don't ask "what topic?" — just run it.
- ONLY ASK when truly required info is missing and you can't infer it.
- CONTEXT MATTERS: If the user's response looks like an answer to your previous question (e.g., short phrases, lists), treat it as such.
- Don't assume recurring unless they say "daily", "every", "schedule", etc.

"#,
            bot_name = bot_name,
            style_desc = style_desc,
            tone_examples = tone_examples,
        );

        // Add user context (framed as background knowledge, not something to announce)
        if user_profile.name.is_some() || user_profile.location.is_some() || user_profile.preferences.is_some() {
            prompt.push_str("## Context About This Person (use naturally, NEVER recite)\n");
            prompt.push_str("You know the following about the user. Use this as background context to be helpful — personalize responses when relevant, but NEVER announce or list this information. A friend knows your name without saying \"Hello [Name] from [City]\" every time.\n");
            if let Some(name) = &user_profile.name {
                prompt.push_str(&format!("- Their name is {}\n", name));
            }
            if let Some(location) = &user_profile.location {
                prompt.push_str(&format!("- They're based in {}\n", location));
            }
            if let Some(prefs) = &user_profile.preferences {
                prompt.push_str(&format!("- They're into {}\n", prefs));
            }
            prompt.push_str("\n");
        }

        // Add onboarding context if not complete
        if !user_profile.onboarding_complete {
            prompt.push_str(r#"## Getting to Know the User
If the user shares personal info (name, location, interests) — either directly or as short answers — note it and respond naturally. Don't repeat it back like a form confirmation. Just acknowledge warmly and move on.
Example: If they say "debanka, kolkata, coding" — respond like "Nice to meet you, Debanka! What are you working on?" NOT "Hello Debanka from Kolkata, I see you like coding."

"#);
        }

        // Note: DID omitted from prompt to save tokens (available via /status API)

        if !memories.is_empty() {
            prompt.push_str("\n## Relevant Memories\n");
            for mem in memories {
                // Truncate long memories to save tokens (UTF-8 safe)
                let content = safe_truncate(&mem.content, 200);
                prompt.push_str(&format!("- {}\n", content));
            }
        }

        // Wrap with security protection against prompt leakage
        Ok(crate::security::SecurityGuard::protect_system_prompt(&prompt))
    }

    /// Execute tool calls from LLM response
    async fn execute_tool_calls(
        &mut self,
        response: &super::llm::LlmResponse,
    ) -> Result<String> {
        if response.tool_calls.is_empty() {
            return Ok(response.content.clone());
        }

        let mut results = Vec::new();

        for call in &response.tool_calls {
            // Check safety policy
            if !self.safety.is_allowed(&call.name, &call.arguments)? {
                results.push(format!(
                    "Tool '{}' blocked by safety policy",
                    call.name
                ));
                continue;
            }

            // Execute in sandbox
            match self.runtime.execute_action(&call.name, &call.arguments).await {
                Ok(result) => {
                    // Special handling for schedule_task - actually create the task
                    if call.name == "schedule_task" && result.starts_with("Task scheduled:") {
                        // Parse the schedule_task result and create actual task
                        if let Some(schedule_result) = self.handle_schedule_task(&call.arguments).await {
                            results.push(schedule_result);
                            continue;
                        }
                    }

                    // Check if this is a workflow action that needs LLM orchestration
                    if result.starts_with("__WORKFLOW_ACTION__:") {
                        let parts: Vec<&str> = result.splitn(3, ':').collect();
                        if parts.len() >= 2 {
                            let action_name = parts[1];
                            let user_query = parts.get(2).unwrap_or(&"");

                            // Get workflow content and execute with LLM
                            if let Some(workflow_content) = self.runtime.get_workflow_content(action_name).await {
                                match self.runtime.execute_workflow_action(
                                    action_name,
                                    &workflow_content,
                                    user_query,
                                    &self.llm,
                                ).await {
                                    Ok(llm_result) => results.push(llm_result),
                                    Err(e) => {
                                        tracing::error!("Workflow action execution error: {}", e);
                                        results.push(format!("Error executing workflow '{}': {}", action_name, e));
                                    }
                                }
                            } else {
                                results.push(format!("Workflow content not found for action: {}", action_name));
                            }
                        }
                    } else {
                        results.push(result);
                    }
                }
                Err(e) => {
                    tracing::error!("Action execution error: {}", e);
                    results.push(format!("Error executing '{}': {}", call.name, e));
                }
            }
        }

        // If there's content plus tool results, combine them
        if response.content.is_empty() {
            Ok(results.join("\n"))
        } else {
            Ok(format!("{}\n\n{}", response.content, results.join("\n")))
        }
    }

    /// Add a task to the autonomous queue
    /// Clear conversation history for a specific channel
    pub async fn clear_conversation_history(&self, channel: &str) {
        let mut history = self.conversation_history.write().await;
        history.remove(channel);
    }

    pub async fn add_task(&self, task: super::task::Task) -> Result<()> {
        let mut queue = self.tasks.write().await;
        self.storage.insert_task(&task).await?;
        queue.add(task);
        Ok(())
    }

    /// Ensure a daily brief task exists (idempotent)
    pub async fn ensure_daily_brief_task(&self) -> Result<()> {
        let existing = self.storage.get("daily_brief_initialized").await?;
        if existing.is_some() {
            return Ok(());
        }

        let tasks = self.storage.get_tasks().await?;
        if tasks.iter().any(|t| t.action == "daily_brief") {
            self.storage.set("daily_brief_initialized", b"true").await?;
            return Ok(());
        }

        // Default daily brief at 9:00 local (cron in 6-field: sec min hour day month weekday)
        let cron_expr = "0 0 9 * * *".to_string();
        let channel = match self.storage.get("daily_brief_channel").await {
            Ok(Some(bytes)) => String::from_utf8(bytes).unwrap_or_else(|_| "telegram".to_string()),
            _ => {
                let _ = self.storage.set("daily_brief_channel", b"telegram").await;
                "telegram".to_string()
            }
        };
        let task = super::task::Task {
            id: uuid::Uuid::new_v4(),
            description: "Daily brief".to_string(),
            action: "daily_brief".to_string(),
            arguments: serde_json::json!({ "report_to": channel }),
            approval: super::task::TaskApproval::Auto,
            capabilities: vec!["daily_brief".to_string()],
            status: super::task::TaskStatus::Pending,
            created_at: chrono::Utc::now(),
            scheduled_for: None,
            cron: Some(cron_expr),
            result: None,
            proof_id: None,
        };

        self.storage.insert_task(&task).await?;
        let mut queue = self.tasks.write().await;
        queue.add(task);
        self.storage.set("daily_brief_initialized", b"true").await?;
        Ok(())
    }

    /// Take due tasks and mark them in-progress
    pub async fn take_due_tasks(&self) -> Vec<super::task::Task> {
        let now = chrono::Utc::now();
        let mut due = Vec::new();
        let mut status_updates: Vec<(String, String)> = Vec::new();
        let mut schedule_updates: Vec<(String, Option<String>, Option<String>)> = Vec::new();
        let tz = {
            let profile = self.user_profile.read().await;
            profile
                .timezone
                .as_deref()
                .and_then(|value| value.parse::<chrono_tz::Tz>().ok())
        };

        {
            let mut tasks = self.tasks.write().await;
            let snapshot = tasks.all().to_vec();
            for task in snapshot.iter() {
                let mut should_run = false;
                let mut next_run: Option<chrono::DateTime<chrono::Utc>> = None;

                if matches!(task.status, super::task::TaskStatus::Pending) {
                    if let Some(ref cron) = task.cron {
                        // If no scheduled_for, compute next run
                        if task.scheduled_for.is_none() {
                            let task_tz = if task.action == "daily_brief" { tz } else { None };
                            next_run = compute_next_run(cron, task_tz);
                        } else if let Some(sf) = task.scheduled_for {
                            if sf <= now {
                                should_run = true;
                            }
                        }
                    } else if let Some(at) = task.scheduled_for {
                        if at <= now {
                            should_run = true;
                        }
                    } else {
                        should_run = true;
                    }
                }

                if let Some(nr) = next_run {
                    if let Some(t) = tasks.get_mut(task.id) {
                        t.scheduled_for = Some(nr);
                        schedule_updates.push((
                            t.id.to_string(),
                            t.cron.clone(),
                            t.scheduled_for.as_ref().map(|d| d.to_rfc3339()),
                        ));
                    }
                }

                if should_run {
                    if let Some(t) = tasks.get_mut(task.id) {
                        t.status = super::task::TaskStatus::InProgress;
                        status_updates.push((
                            t.id.to_string(),
                            serde_json::to_string(&t.status).unwrap_or_else(|_| "InProgress".to_string()),
                        ));
                        due.push(t.clone());
                    }
                }
            }
        }

        for (id, status) in status_updates {
            let _ = self.storage.update_task_status(&id, &status).await;
        }
        for (id, cron, scheduled_for) in schedule_updates {
            let _ = self.storage.update_task(&id, None, None, cron, scheduled_for).await;
        }

        due
    }

    /// Execute a task (plan or single action) and return output
    pub async fn execute_task(&self, task: &super::task::Task) -> Result<String> {
        if task.action == "daily_brief" {
            return self.build_daily_brief().await;
        }

        if task.action == "plan" {
            let steps = task
                .arguments
                .get("steps")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow::anyhow!("Plan task missing steps"))?;

            let mut outputs = Vec::new();
            for step in steps {
                let action_name = step
                    .get("action")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Plan step missing action"))?;
                let args = step.get("arguments").cloned().unwrap_or_else(|| serde_json::json!({}));

                if !self.safety.is_allowed(action_name, &args)? {
                    outputs.push(format!("Tool '{}' blocked by safety policy", action_name));
                    continue;
                }

                let result = self.runtime.execute_action(action_name, &args).await?;
                let handled = if result.starts_with("__WORKFLOW_ACTION__:") {
                    let parts: Vec<&str> = result.splitn(3, ':').collect();
                    if parts.len() >= 2 {
                        let wf_action_name = parts[1];
                        let user_query = parts.get(2).unwrap_or(&"");
                        if let Some(workflow_content) = self.runtime.get_workflow_content(wf_action_name).await {
                            self.runtime.execute_workflow_action(
                                wf_action_name,
                                &workflow_content,
                                user_query,
                                &self.llm,
                            ).await?
                        } else {
                            format!("Workflow content not found for action: {}", wf_action_name)
                        }
                    } else {
                        result
                    }
                } else {
                    result
                };
                outputs.push(handled);
            }
            return Ok(outputs.join("\n\n"));
        }

        let result = self.runtime.execute_action(&task.action, &task.arguments).await?;
        if result.starts_with("__WORKFLOW_ACTION__:") {
            let parts: Vec<&str> = result.splitn(3, ':').collect();
            if parts.len() >= 2 {
                let wf_action_name = parts[1];
                let user_query = parts.get(2).unwrap_or(&"");
                if let Some(workflow_content) = self.runtime.get_workflow_content(wf_action_name).await {
                    return self.runtime.execute_workflow_action(
                        wf_action_name,
                        &workflow_content,
                        user_query,
                        &self.llm,
                    ).await;
                }
            }
        }
        Ok(result)
    }

    /// Update task result and status
    pub async fn finalize_task(
        &self,
        id: uuid::Uuid,
        status: super::task::TaskStatus,
        result: Option<String>,
    ) -> Result<()> {
        let mut stored_status = status.clone();
        let mut schedule_update: Option<(Option<String>, Option<String>)> = None;
        let tz = {
            let profile = self.user_profile.read().await;
            profile
                .timezone
                .as_deref()
                .and_then(|value| value.parse::<chrono_tz::Tz>().ok())
        };

        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(id) {
                if task.cron.is_some() && matches!(status, super::task::TaskStatus::Completed) {
                    let task_tz = if task.action == "daily_brief" { tz } else { None };
                    task.scheduled_for = task
                        .cron
                        .as_deref()
                        .and_then(|cron| compute_next_run(cron, task_tz));
                    stored_status = super::task::TaskStatus::Pending;
                }
                task.status = stored_status.clone();
                task.result = result.clone();
                if task.cron.is_some() {
                    schedule_update = Some((
                        task.cron.clone(),
                        task.scheduled_for.as_ref().map(|d| d.to_rfc3339()),
                    ));
                }
            }
        }

        let status_json = serde_json::to_string(&stored_status).unwrap_or_else(|_| "Completed".to_string());
        self.storage
            .update_task_status_and_result(&id.to_string(), &status_json, result.as_deref())
            .await?;

        if let Some((cron, scheduled_for)) = schedule_update {
            let _ = self.storage.update_task(&id.to_string(), None, None, cron, scheduled_for).await;
        }

        Ok(())
    }

    async fn build_daily_brief(&self) -> Result<String> {
        let tasks = self.tasks.read().await;
        let pending = tasks.all().iter()
            .filter(|t| matches!(t.status, super::task::TaskStatus::Pending | super::task::TaskStatus::AwaitingApproval))
            .take(10)
            .map(|t| format!("- {}{}", t.description, t.cron.as_ref().map(|c| format!(" (cron: {})", c)).unwrap_or_default()))
            .collect::<Vec<_>>()
            .join("\n");

        let trace = self.trace_history.read().await;
        let recent = trace.iter().rev().take(3)
            .map(|t| format!("- {} ({})", t.message, t.completed_at.map(|d| d.format("%H:%M").to_string()).unwrap_or_else(|| "pending".to_string())))
            .collect::<Vec<_>>()
            .join("\n");

        let profile = self.user_profile.read().await;
        let mut style = Vec::new();
        if let Some(lang) = profile.language.as_ref().filter(|v| !v.trim().is_empty()) {
            style.push(format!("Language: {}", lang.trim()));
        }
        if let Some(tone) = profile.tone.as_ref().filter(|v| !v.trim().is_empty()) {
            style.push(format!("Tone: {}", tone.trim()));
        }
        if let Some(format) = profile.email_format.as_ref().filter(|v| !v.trim().is_empty()) {
            style.push(format!("Format: {}", format.trim()));
        }
        let style_block = if style.is_empty() {
            "Use a neutral, helpful tone.".to_string()
        } else {
            style.join(" | ")
        };

        let prompt = format!(
            "Create a concise daily brief for the user.\n{}\n\nPending tasks:\n{}\n\nRecent activity:\n{}\n\nWrite 5-8 bullet points max.",
            style_block,
            if pending.is_empty() { "None" } else { &pending },
            if recent.is_empty() { "None" } else { &recent }
        );

        let empty_actions: Vec<crate::actions::ActionDef> = Vec::new();
        let response = self.llm.chat(
            "You are a concise assistant creating daily briefs.",
            &prompt,
            &[],
            &empty_actions,
        ).await?;

        Ok(response.content)
    }

    /// Handle schedule_task tool call - actually create the scheduled task
    async fn handle_schedule_task(&self, arguments: &serde_json::Value) -> Option<String> {
        let task_desc = arguments.get("task")?.as_str()?;

        // Parse cron or at time
        let (cron_expr, scheduled_for) = if let Some(cron) = arguments.get("cron").and_then(|v| v.as_str()) {
            // Convert 5-field cron to 6-field (with seconds)
            let cron_6field = if cron.split_whitespace().count() == 5 {
                format!("0 {}", cron)
            } else {
                cron.to_string()
            };
            (Some(cron_6field), None)
        } else if let Some(at_time) = arguments.get("at").and_then(|v| v.as_str()) {
            let dt = chrono::DateTime::parse_from_rfc3339(at_time).ok()?;
            (None, Some(dt.with_timezone(&chrono::Utc)))
        } else {
            return None;
        };

        // Try to detect the action from the task description
        let task_lower = task_desc.to_lowercase();
        let action_name = if task_lower.contains("trend") && task_lower.contains("prophet") {
            "trend-prophet".to_string()
        } else if task_lower.contains("market") && task_lower.contains("analysis") {
            "market-analysis".to_string()
        } else if task_lower.contains("research") {
            "research".to_string()
        } else if task_lower.contains("search") {
            "web_search".to_string()
        } else {
            // Default to the task description itself as a generic task
            "generic".to_string()
        };

        // Build task arguments
        let task_args = serde_json::json!({
            "query": task_desc,
            "report_to": "telegram"
        });

        let task = super::task::Task {
            id: uuid::Uuid::new_v4(),
            description: task_desc.to_string(),
            action: action_name.clone(),
            arguments: task_args,
            approval: super::task::TaskApproval::Auto,
            capabilities: vec![action_name.clone()],
            status: super::task::TaskStatus::Pending,
            created_at: chrono::Utc::now(),
            scheduled_for,
            cron: cron_expr.clone(),
            result: None,
            proof_id: None,
        };

        // Add to queue
        let mut queue = self.tasks.write().await;
        if let Err(e) = self.storage.insert_task(&task).await {
            tracing::error!("Failed to save scheduled task: {}", e);
            return Some(format!("Failed to schedule task: {}", e));
        }
        queue.add(task);

        let schedule_desc = if let Some(ref cron) = cron_expr {
            format!("recurring (cron: {})", cron)
        } else if let Some(at) = scheduled_for {
            format!("one-time at {}", at.format("%Y-%m-%d %H:%M"))
        } else {
            "unknown".to_string()
        };

        Some(format!("✅ Task scheduled successfully!\n\n📋 **Task**: {}\n🔧 **Action**: {}\n⏰ **Schedule**: {}\n📱 **Report to**: Telegram",
            task_desc, action_name, schedule_desc))
    }

    /// Get agent status
    pub async fn status(&self) -> AgentStatus {
        let tasks = self.tasks.read().await;
        let pending_count = tasks.all()
            .iter()
            .filter(|t| matches!(t.status, super::task::TaskStatus::Pending | super::task::TaskStatus::AwaitingApproval))
            .count();

        AgentStatus {
            did: self.identity.did().to_string(),
            memory_entries: self.memory.entry_count(),
            actions_loaded: self.runtime.action_count().await,
            tasks_pending: pending_count,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentStatus {
    pub did: String,
    pub memory_entries: usize,
    pub actions_loaded: usize,
    pub tasks_pending: usize,
}

fn compute_next_run(cron_expr: &str, tz: Option<chrono_tz::Tz>) -> Option<chrono::DateTime<chrono::Utc>> {
    let schedule = cron_expr.parse::<cron::Schedule>().ok()?;
    match tz {
        Some(tz) => schedule.upcoming(tz).next().map(|dt| dt.with_timezone(&chrono::Utc)),
        None => schedule.upcoming(chrono::Utc).next(),
    }
}
