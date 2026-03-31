//! External Service Integrations
//!
//! Connects AgentArk to external services like Google Calendar, WhatsApp, etc.
//! Each integration implements the `Integration` trait for unified handling.

pub mod browser;
pub mod calendar;
pub mod ga4;
pub mod garmin;
pub mod github;
pub mod gsc;
pub mod lightpanda;
pub mod media_gen;
pub mod mem0;
pub mod moltbook;
pub mod notion;
pub mod oauth;
pub mod onepassword;
pub mod ordering;
pub mod places;
pub mod social_analytics;
pub mod twilio;
pub mod twitter;
pub mod whatsapp;
pub mod whoop;

use anyhow::Result;
use async_trait::async_trait;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

const INTEGRATION_STATUS_TIMEOUT: Duration = Duration::from_secs(4);

fn parse_boolish(value: &str) -> Option<bool> {
    let v = value.trim().to_ascii_lowercase();
    if v.is_empty() {
        return None;
    }
    match v.as_str() {
        "1" | "true" | "yes" | "y" | "on" => Some(true),
        "0" | "false" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}

pub fn integration_enabled_key(id: &str) -> String {
    format!("integration_enabled:{}", id)
}

pub fn integration_user_disabled_key(id: &str) -> String {
    format!("integration_user_disabled:{}", id)
}

fn stored_bool_secret(
    manager: &crate::core::config::SecureConfigManager,
    key: &str,
) -> Option<bool> {
    manager
        .get_custom_secret(key)
        .ok()
        .flatten()
        .and_then(|value| parse_boolish(&value))
}

fn legacy_google_refresh_token_present(
    manager: &crate::core::config::SecureConfigManager,
    key: &str,
) -> bool {
    manager
        .get_custom_secret(key)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|parsed| {
            parsed
                .get("refresh_token")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .is_some()
}

fn builtin_integration_is_connected(
    config_dir: &Path,
    manager: &crate::core::config::SecureConfigManager,
    integration_id: &str,
) -> bool {
    match integration_id {
        "gmail" => legacy_google_refresh_token_present(manager, "gmail_tokens"),
        "google_calendar" => legacy_google_refresh_token_present(manager, "calendar_tokens"),
        "google_workspace" => {
            crate::actions::google_workspace::summarize_connection_status(config_dir)
                .map(|(connected, granted, missing)| {
                    connected && !granted.is_empty() && missing.is_empty()
                })
                .unwrap_or(false)
        }
        _ => false,
    }
}

pub fn effective_integration_enabled(config_dir: &Path, integration_id: &str) -> bool {
    let Ok(manager) = crate::core::config::SecureConfigManager::new(config_dir) else {
        return true;
    };

    if stored_bool_secret(&manager, &integration_user_disabled_key(integration_id)).unwrap_or(false)
    {
        return false;
    }

    let explicit = stored_bool_secret(&manager, &integration_enabled_key(integration_id));
    if !matches!(
        integration_id,
        "gmail" | "google_calendar" | "google_workspace"
    ) {
        return explicit.unwrap_or(true);
    }

    if explicit == Some(true) {
        return true;
    }

    if builtin_integration_is_connected(config_dir, &manager, integration_id) {
        let _ = manager.set_custom_secret(
            &integration_enabled_key(integration_id),
            Some("true".to_string()),
        );
        return true;
    }

    explicit.unwrap_or(true)
}

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
    /// Can send notifications/messages to the user
    Notify,
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
pub struct IntegrationManager {
    integrations: HashMap<String, Box<dyn Integration>>,
    _config_dir: std::path::PathBuf,
}

impl IntegrationManager {
    pub fn new(config_dir: &std::path::Path) -> Self {
        let mut manager = Self {
            integrations: HashMap::new(),
            _config_dir: config_dir.to_path_buf(),
        };

        // Register available integrations
        manager.register_default_integrations();
        manager
    }

    /// Register default integrations (Google Calendar, WhatsApp, Media Gen)
    fn register_default_integrations(&mut self) {
        let config_dir = self._config_dir.clone();

        // Register Google Calendar
        let calendar = calendar::GoogleCalendarConnector::new();
        self.integrations
            .insert("google_calendar".to_string(), Box::new(calendar));

        // Register WhatsApp
        let whatsapp = whatsapp::WhatsAppConnector::new();
        self.integrations
            .insert("whatsapp".to_string(), Box::new(whatsapp));

        // Register AI Media Generation (Image/Video)
        let media_gen = media_gen::MediaGenConnector::new();
        self.integrations
            .insert("media_gen".to_string(), Box::new(media_gen));

        // Register GitHub
        let github = github::GitHubConnector::new_with_config_dir(config_dir.clone());
        self.integrations
            .insert("github".to_string(), Box::new(github));

        // Register Notion
        let notion = notion::NotionConnector::new_with_config_dir(config_dir.clone());
        self.integrations
            .insert("notion".to_string(), Box::new(notion));

        // Register Twitter/X
        let twitter = twitter::TwitterConnector::new_with_config_dir(config_dir.clone());
        self.integrations
            .insert("twitter".to_string(), Box::new(twitter));

        // Register 1Password
        let onepassword =
            onepassword::OnePasswordConnector::new_with_config_dir(config_dir.clone());
        self.integrations
            .insert("onepassword".to_string(), Box::new(onepassword));

        // Register Google Places
        let places = places::GooglePlacesConnector::new_with_config_dir(config_dir.clone());
        self.integrations
            .insert("google_places".to_string(), Box::new(places));

        // Register Twilio Voice & SMS
        let twilio = twilio::TwilioConnector::new_with_config_dir(config_dir.clone());
        self.integrations
            .insert("twilio".to_string(), Box::new(twilio));

        // Register Ordering & Purchasing
        let ordering = ordering::OrderingConnector::new_with_config_dir(config_dir.clone());
        self.integrations
            .insert("ordering".to_string(), Box::new(ordering));

        // Register Browser Automation (Playwright sidecar)
        let browser = browser::BrowserIntegration::new();
        self.integrations
            .insert("browser".to_string(), Box::new(browser));

        // Curated health + analytics connectors
        let garmin = garmin::GarminConnector::new_with_config_dir(config_dir.clone());
        self.integrations
            .insert("garmin".to_string(), Box::new(garmin));

        let whoop = whoop::WhoopConnector::new_with_config_dir(config_dir.clone());
        self.integrations
            .insert("whoop".to_string(), Box::new(whoop));

        let ga4 = ga4::Ga4Connector::new_with_config_dir(config_dir.clone());
        self.integrations.insert("ga4".to_string(), Box::new(ga4));

        let gsc = gsc::GscConnector::new_with_config_dir(config_dir.clone());
        self.integrations.insert("gsc".to_string(), Box::new(gsc));

        let social = social_analytics::SocialAnalyticsConnector::new_with_config_dir(config_dir);
        self.integrations
            .insert("social_analytics".to_string(), Box::new(social));

        // Register Moltbook
        let moltbook = moltbook::MoltbookConnector::new_with_config_dir(self._config_dir.clone());
        self.integrations
            .insert("moltbook".to_string(), Box::new(moltbook));
    }

    /// Get an integration by ID
    pub fn get(&self, id: &str) -> Option<&dyn Integration> {
        self.integrations.get(id).map(|i| i.as_ref())
    }

    /// List registered integration IDs.
    pub fn ids(&self) -> Vec<String> {
        self.integrations.keys().cloned().collect()
    }

    /// Returns true when an integration is enabled for agent dispatch.
    /// Missing/invalid flags default to enabled (matching execute-time behavior).
    pub fn is_enabled(&self, integration_id: &str) -> bool {
        effective_integration_enabled(&self._config_dir, integration_id)
    }

    /// List integration IDs that are currently enabled for agent dispatch.
    pub fn enabled_ids(&self) -> Vec<String> {
        self.integrations
            .keys()
            .filter(|id| effective_integration_enabled(&self._config_dir, id.as_str()))
            .cloned()
            .collect()
    }

    /// List all integrations with their status
    pub async fn list(&self) -> Vec<IntegrationInfo> {
        join_all(
            self.integrations
                .iter()
                .map(|(id, integration)| async move {
                    let status = match tokio::time::timeout(
                        INTEGRATION_STATUS_TIMEOUT,
                        integration.status(),
                    )
                    .await
                    {
                        Ok(status) => status,
                        Err(_) => IntegrationStatus::Error("Status check timed out".to_string()),
                    };
                    IntegrationInfo {
                        id: id.clone(),
                        name: integration.name().to_string(),
                        description: integration.description().to_string(),
                        icon: integration.icon().to_string(),
                        capabilities: integration.capabilities(),
                        status,
                    }
                }),
        )
        .await
    }

    /// Return all connected integrations that support notifications
    pub async fn notifiable_integrations(&self) -> Vec<String> {
        let mut result = Vec::new();
        for (id, integration) in &self.integrations {
            if integration.capabilities().contains(&Capability::Notify)
                && self.is_enabled(id)
                && integration.is_connected().await
            {
                result.push(id.clone());
            }
        }
        result
    }

    /// Execute an action on an integration
    pub async fn execute(
        &self,
        integration_id: &str,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Enforce user enable/disable toggle (stored in encrypted secrets).
        if !self.is_enabled(integration_id) {
            return Err(anyhow::anyhow!(
                "Integration '{}' is disabled",
                integration_id
            ));
        }

        let integration = self
            .integrations
            .get(integration_id)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_integration_enabled_autoheals_connected_google_workspace() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager =
            crate::core::config::SecureConfigManager::new(dir.path()).expect("secure manager");
        manager
            .set_custom_secret(
                crate::actions::google_workspace::GOOGLE_WORKSPACE_TOKENS_KEY,
                Some(
                    serde_json::json!({
                        "access_token": "access",
                        "refresh_token": "refresh",
                        "expires_at": chrono::Utc::now().timestamp() + 3600,
                        "granted_scopes": [
                            "https://www.googleapis.com/auth/gmail.readonly",
                            "https://www.googleapis.com/auth/gmail.send",
                            "https://www.googleapis.com/auth/calendar"
                        ],
                        "granted_bundles": ["gmail", "calendar"]
                    })
                    .to_string(),
                ),
            )
            .expect("workspace tokens saved");
        manager
            .set_custom_secret(
                crate::actions::google_workspace::GOOGLE_WORKSPACE_BUNDLES_KEY,
                Some(serde_json::json!(["gmail", "calendar"]).to_string()),
            )
            .expect("workspace bundles saved");
        manager
            .set_custom_secret(
                &integration_enabled_key("google_workspace"),
                Some("false".to_string()),
            )
            .expect("stale disabled flag saved");

        assert!(effective_integration_enabled(
            dir.path(),
            "google_workspace"
        ));
        assert_eq!(
            manager
                .get_custom_secret(&integration_enabled_key("google_workspace"))
                .expect("load enabled flag")
                .as_deref(),
            Some("true")
        );
    }

    #[test]
    fn effective_integration_enabled_respects_manual_disable_marker() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager =
            crate::core::config::SecureConfigManager::new(dir.path()).expect("secure manager");
        manager
            .set_custom_secret(
                crate::actions::google_workspace::GOOGLE_WORKSPACE_TOKENS_KEY,
                Some(
                    serde_json::json!({
                        "access_token": "access",
                        "refresh_token": "refresh",
                        "expires_at": chrono::Utc::now().timestamp() + 3600,
                        "granted_scopes": [
                            "https://www.googleapis.com/auth/gmail.readonly",
                            "https://www.googleapis.com/auth/gmail.send"
                        ],
                        "granted_bundles": ["gmail"]
                    })
                    .to_string(),
                ),
            )
            .expect("workspace tokens saved");
        manager
            .set_custom_secret(
                &integration_user_disabled_key("google_workspace"),
                Some("true".to_string()),
            )
            .expect("manual disable marker saved");
        manager
            .set_custom_secret(
                &integration_enabled_key("google_workspace"),
                Some("false".to_string()),
            )
            .expect("disabled flag saved");

        assert!(!effective_integration_enabled(
            dir.path(),
            "google_workspace"
        ));
        assert_eq!(
            manager
                .get_custom_secret(&integration_enabled_key("google_workspace"))
                .expect("load enabled flag")
                .as_deref(),
            Some("false")
        );
    }
}
