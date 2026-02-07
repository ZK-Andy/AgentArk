//! External Service Integrations
//!
//! Connects CogniArk to external services like Google Calendar, WhatsApp, etc.
//! Each integration implements the `Integration` trait for unified handling.

pub mod calendar;
pub mod oauth;
pub mod whatsapp;
pub mod media_gen;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Capabilities an integration can provide
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    /// Can read data
    Read,
    /// Can write/create data
    Write,
    /// Can subscribe to updates (webhooks)
    Subscribe,
    /// Can search/query data
    Search,
    /// Can delete data
    Delete,
}

/// Integration status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegrationStatus {
    /// Not configured
    NotConfigured,
    /// Configured but not connected (needs OAuth)
    NeedsAuth,
    /// Connected and working
    Connected,
    /// Connection error
    Error(String),
}

/// Base trait for all integrations
#[async_trait]
#[allow(dead_code)]
pub trait Integration: Send + Sync {
    /// Unique identifier for this integration
    fn id(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Description of what this integration does
    fn description(&self) -> &str;

    /// Icon/emoji for UI
    fn icon(&self) -> &str;

    /// What this integration can do
    fn capabilities(&self) -> Vec<Capability>;

    /// Current status
    async fn status(&self) -> IntegrationStatus;

    /// Check if the integration is ready to use
    async fn is_connected(&self) -> bool {
        matches!(self.status().await, IntegrationStatus::Connected)
    }

    /// Execute an action
    async fn execute(&self, action: &str, params: &serde_json::Value) -> Result<serde_json::Value>;

    /// Handle incoming webhook (if supported)
    async fn handle_webhook(&self, _payload: &serde_json::Value) -> Result<()> {
        Ok(()) // Default: no-op
    }
}

/// Integration manager - holds all configured integrations
#[allow(dead_code)]
pub struct IntegrationManager {
    integrations: HashMap<String, Box<dyn Integration>>,
    config_dir: std::path::PathBuf,
}

impl IntegrationManager {
    pub fn new(config_dir: &std::path::Path) -> Self {
        let mut manager = Self {
            integrations: HashMap::new(),
            config_dir: config_dir.to_path_buf(),
        };

        // Register available integrations
        manager.register_default_integrations();
        manager
    }

    /// Register default integrations (Google Calendar, WhatsApp, Media Gen)
    fn register_default_integrations(&mut self) {
        // Register Google Calendar
        let calendar = calendar::GoogleCalendarConnector::new();
        self.integrations.insert("google_calendar".to_string(), Box::new(calendar));

        // Register WhatsApp
        let whatsapp = whatsapp::WhatsAppConnector::new();
        self.integrations.insert("whatsapp".to_string(), Box::new(whatsapp));

        // Register AI Media Generation (Image/Video)
        let media_gen = media_gen::MediaGenConnector::new();
        self.integrations.insert("media_gen".to_string(), Box::new(media_gen));
    }

    /// Register an integration
    #[allow(dead_code)]
    pub fn register(&mut self, integration: Box<dyn Integration>) {
        let id = integration.id().to_string();
        self.integrations.insert(id, integration);
    }

    /// Get an integration by ID
    pub fn get(&self, id: &str) -> Option<&dyn Integration> {
        self.integrations.get(id).map(|i| i.as_ref())
    }

    /// List all integrations with their status
    pub async fn list(&self) -> Vec<IntegrationInfo> {
        let mut result = Vec::new();
        for (id, integration) in &self.integrations {
            result.push(IntegrationInfo {
                id: id.clone(),
                name: integration.name().to_string(),
                description: integration.description().to_string(),
                icon: integration.icon().to_string(),
                capabilities: integration.capabilities(),
                status: integration.status().await,
            });
        }
        result
    }

    /// Execute an action on an integration
    #[allow(dead_code)]
    pub async fn execute(&self, integration_id: &str, action: &str, params: &serde_json::Value) -> Result<serde_json::Value> {
        let integration = self.integrations.get(integration_id)
            .ok_or_else(|| anyhow::anyhow!("Integration '{}' not found", integration_id))?;

        integration.execute(action, params).await
    }
}

/// Info about an integration for API/UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub capabilities: Vec<Capability>,
    pub status: IntegrationStatus,
}
