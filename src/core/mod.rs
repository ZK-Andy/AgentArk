//! Core agent module - the brain of Crate Agent

mod agent;
mod llm;
pub mod orchestra;
pub mod parallel;
mod task;
pub mod config;

pub use agent::{Agent, ExecutionTrace, UserProfile};
pub use llm::{LlmClient, LlmProvider, ToolCall};
pub use task::{Task, TaskQueue, TaskStatus, TaskApproval};
pub use config::AgentConfig;
