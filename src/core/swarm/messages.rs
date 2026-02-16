//! Message types for inter-agent delegation

use super::agent_trait::AgentId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A delegation request from coordinator to specialist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationRequest {
    pub task_id: Uuid,
    pub description: String,
    pub context: String,
    pub constraints: Option<String>,
    pub timeout_secs: u64,
    pub parent_task_id: Option<Uuid>,
}

/// A delegation result from specialist back to coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationResult {
    pub task_id: Uuid,
    pub agent_id: AgentId,
    pub agent_name: String,
    pub success: bool,
    pub content: String,
    pub confidence: f32,
    pub execution_time_ms: u64,
    pub error: Option<String>,
}
