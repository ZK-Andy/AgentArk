//! Task queue for autonomous execution

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Task approval policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskApproval {
    /// Execute immediately without approval
    Auto,
    /// Notify user, wait for duration, then execute
    NotifyThenExecute { delay_seconds: u64 },
    /// Require explicit user approval
    RequireApproval,
}

/// Task status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    AwaitingApproval,
    InProgress,
    Completed,
    Failed { error: String },
    Cancelled,
}

/// A task for the agent to execute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub description: String,
    pub action: String,
    pub arguments: serde_json::Value,
    pub approval: TaskApproval,
    pub capabilities: Vec<String>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub scheduled_for: Option<DateTime<Utc>>,
    pub cron: Option<String>,
    pub result: Option<String>,
    pub proof_id: Option<Uuid>,
}

impl Task {
    #[allow(dead_code)]
    pub fn new(description: String, action: String, arguments: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            description,
            action,
            arguments,
            approval: TaskApproval::Auto,
            capabilities: vec![],
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            scheduled_for: None,
            cron: None,
            result: None,
            proof_id: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_approval(mut self, approval: TaskApproval) -> Self {
        self.approval = approval;
        self
    }

    #[allow(dead_code)]
    pub fn requires_approval(&self) -> bool {
        matches!(self.approval, TaskApproval::RequireApproval)
    }
}

/// Queue of tasks for autonomous execution
pub struct TaskQueue {
    tasks: Vec<Task>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self { tasks: vec![] }
    }

    pub fn add(&mut self, task: Task) {
        self.tasks.push(task);
    }

    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    pub fn remove(&mut self, id: Uuid) -> bool {
        let before = self.tasks.len();
        self.tasks.retain(|t| t.id != id);
        before != self.tasks.len()
    }

    pub fn all(&self) -> &[Task] {
        &self.tasks
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}
