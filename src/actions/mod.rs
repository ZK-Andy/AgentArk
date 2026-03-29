//! Action system with self-improvement capabilities
//!
//! Based on arXiv:2512.17102 "SAGE: Self-Improving Agent with Action Library"

pub mod app;
pub mod calendar;
pub mod gmail;
pub mod google_workspace;
pub mod research;
pub mod search;
#[cfg(feature = "ssh")]
pub mod ssh;
pub mod video;

use serde::{Deserialize, Serialize};

use crate::runtime::SandboxMode;

#[allow(unused_imports)]
pub use gmail::{gmail_reply, gmail_scan};
#[allow(unused_imports)]
pub use research::{execute_research, ResearchArgs, ResearchClient, ResearchDepth, ResearchResult};
#[allow(unused_imports)]
pub use search::{SearchBackend, SearchClient, SearchConfig, SearchResponse, SearchResult};

/// Action source type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionSource {
    /// Built-in system action (not editable)
    System,
    /// Bundled workflow action (editable)
    Bundled,
    /// User-created custom action (editable)
    Custom,
}

/// Information about an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDef {
    /// Action name (unique identifier)
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// Action version
    pub version: String,

    /// JSON Schema for input parameters
    pub input_schema: serde_json::Value,

    /// Required capabilities
    pub capabilities: Vec<String>,

    /// Preferred sandbox mode
    pub sandbox_mode: Option<SandboxMode>,

    /// Action source (system, bundled, or custom)
    #[serde(default = "default_action_source")]
    pub source: ActionSource,

    /// Path to action file (for editable actions)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

impl Default for ActionDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({}),
            capabilities: vec![],
            sandbox_mode: None,
            source: ActionSource::System,
            file_path: None,
        }
    }
}

fn default_action_source() -> ActionSource {
    ActionSource::System
}
