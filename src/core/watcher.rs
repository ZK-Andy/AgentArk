//! Background watcher system — poll-until-condition-then-act
//!
//! Allows the agent to spawn short-lived background watchers that:
//! 1. Poll an action (e.g. gmail_scan) at a regular interval
//! 2. Check a condition against the result (e.g. "not empty", "contains keyword")
//! 3. When triggered: execute a chain of follow-up actions via the agent
//! 4. Self-terminate after trigger or timeout
//!
//! Watchers are persisted to disk so they survive container restarts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Default max watch duration: 3 hours
pub const DEFAULT_TIMEOUT_SECS: u64 = 10800;

/// Max allowed timeout: 24 hours
pub const MAX_TIMEOUT_SECS: u64 = 86400;

/// Status of a watcher
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WatcherStatus {
    /// Actively polling
    Active,
    /// Condition was met — follow-up actions queued
    Triggered,
    /// Timed out without finding a match
    TimedOut,
    /// Cancelled by user
    Cancelled,
    /// Error during polling
    Failed { error: String },
}

/// Condition to evaluate against poll results
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchCondition {
    /// Result is not empty / not "No messages found" etc.
    NotEmpty,
    /// Result contains a keyword (case-insensitive)
    Contains { keyword: String },
    /// Result matches a regex pattern
    Matches { pattern: String },
    /// Custom condition described in natural language (evaluated by LLM)
    Custom { description: String },
}

impl WatchCondition {
    /// Evaluate the condition against a poll result
    pub fn evaluate(&self, result: &str) -> bool {
        let trimmed = result.trim();
        match self {
            WatchCondition::NotEmpty => {
                !trimmed.is_empty()
                    && !trimmed.eq_ignore_ascii_case("no messages found.")
                    && !trimmed.eq_ignore_ascii_case("no results")
                    && !trimmed.eq_ignore_ascii_case("no results found")
                    && !trimmed.starts_with("Error")
            }
            WatchCondition::Contains { keyword } => {
                trimmed.to_lowercase().contains(&keyword.to_lowercase())
            }
            WatchCondition::Matches { pattern } => regex::Regex::new(pattern)
                .map(|re| re.is_match(trimmed))
                .unwrap_or(false),
            WatchCondition::Custom { .. } => {
                // Custom conditions need LLM evaluation — treated as NotEmpty
                // for the poll loop. The agent re-evaluates with LLM after trigger.
                !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("no messages found.")
            }
        }
    }
}

/// A background watcher definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Watcher {
    /// Unique watcher ID
    pub id: Uuid,
    /// Human-readable description of what this watcher does
    pub description: String,
    /// Action to poll (e.g. "gmail_scan")
    pub poll_action: String,
    /// Arguments for the poll action
    pub poll_arguments: serde_json::Value,
    /// Condition that triggers the watcher
    pub condition: WatchCondition,
    /// What to do when triggered — described in natural language for the agent
    pub on_trigger: String,
    /// Polling interval in seconds (default: 60)
    pub interval_secs: u64,
    /// Maximum time to watch in seconds (default: 10800 = 3 hours)
    pub timeout_secs: u64,
    /// Channel to notify when triggered or timed out
    pub notify_channel: String,
    /// Current status
    pub status: WatcherStatus,
    /// When the watcher was created
    pub created_at: DateTime<Utc>,
    /// When the watcher last polled
    pub last_poll_at: Option<DateTime<Utc>>,
    /// Number of polls executed
    pub poll_count: u32,
    /// The result that triggered the watcher (if triggered)
    pub trigger_result: Option<String>,
}

/// Manages all active watchers with persistent storage
pub struct WatcherManager {
    watchers: Arc<RwLock<HashMap<Uuid, Watcher>>>,
    storage_path: Option<PathBuf>,
}

impl WatcherManager {
    pub fn new(data_dir: Option<&std::path::Path>) -> Self {
        let storage_path = data_dir.map(|d| d.join("watchers.json"));

        // Load persisted watchers
        let watchers = if let Some(ref path) = storage_path {
            match std::fs::read_to_string(path) {
                Ok(contents) => {
                    match serde_json::from_str::<HashMap<Uuid, Watcher>>(&contents) {
                        Ok(mut loaded) => {
                            // Only restore Active watchers that haven't timed out
                            let now = Utc::now();
                            loaded.retain(|_, w| {
                                if w.status != WatcherStatus::Active {
                                    return false;
                                }
                                let elapsed = (now - w.created_at).num_seconds() as u64;
                                elapsed < w.timeout_secs
                            });
                            let count = loaded.len();
                            if count > 0 {
                                tracing::info!("Restored {} active watcher(s) from disk", count);
                            }
                            loaded
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse watchers.json: {}", e);
                            HashMap::new()
                        }
                    }
                }
                Err(_) => HashMap::new(), // File doesn't exist yet
            }
        } else {
            HashMap::new()
        };

        Self {
            watchers: Arc::new(RwLock::new(watchers)),
            storage_path,
        }
    }

    /// Persist current watchers to disk
    fn save_sync(path: &std::path::Path, watchers: &HashMap<Uuid, Watcher>) {
        // Only persist Active watchers — completed ones get cleaned up
        let active: HashMap<&Uuid, &Watcher> = watchers
            .iter()
            .filter(|(_, w)| w.status == WatcherStatus::Active)
            .collect();

        if let Ok(json) = serde_json::to_string_pretty(&active) {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(path, json) {
                tracing::warn!("Failed to save watchers: {}", e);
            }
        }
    }

    /// Save watchers to disk (only Active ones)
    async fn persist(&self) {
        if let Some(ref path) = self.storage_path {
            let watchers = self.watchers.read().await;
            Self::save_sync(path, &watchers);
        }
    }

    /// Add a new watcher and return its ID
    pub async fn add(&self, watcher: Watcher) -> Uuid {
        let id = watcher.id;
        self.watchers.write().await.insert(id, watcher);
        self.persist().await;
        id
    }

    /// Get all watchers
    pub async fn list(&self) -> Vec<Watcher> {
        self.watchers.read().await.values().cloned().collect()
    }

    /// Get active watchers that need polling
    pub async fn get_due_watchers(&self) -> Vec<Watcher> {
        let now = Utc::now();
        let watchers = self.watchers.read().await;
        watchers
            .values()
            .filter(|w| {
                if w.status != WatcherStatus::Active {
                    return false;
                }
                // Check timeout
                let elapsed = (now - w.created_at).num_seconds() as u64;
                if elapsed >= w.timeout_secs {
                    return false; // Will be timed out in tick()
                }
                // Check if enough time has passed since last poll
                match w.last_poll_at {
                    Some(last) => (now - last).num_seconds() as u64 >= w.interval_secs,
                    None => true, // Never polled — poll immediately
                }
            })
            .cloned()
            .collect()
    }

    /// Update a watcher after polling
    pub async fn update_poll(&self, id: Uuid, poll_count: u32) {
        if let Some(w) = self.watchers.write().await.get_mut(&id) {
            w.last_poll_at = Some(Utc::now());
            w.poll_count = poll_count;
        }
        self.persist().await;
    }

    /// Mark a watcher as triggered
    pub async fn mark_triggered(&self, id: Uuid, result: String) {
        if let Some(w) = self.watchers.write().await.get_mut(&id) {
            w.status = WatcherStatus::Triggered;
            w.trigger_result = Some(result);
        }
        self.persist().await;
    }

    /// Mark timed-out watchers
    pub async fn expire_watchers(&self) -> Vec<Watcher> {
        let now = Utc::now();
        let mut expired = Vec::new();
        let mut watchers = self.watchers.write().await;
        for w in watchers.values_mut() {
            if w.status == WatcherStatus::Active {
                let elapsed = (now - w.created_at).num_seconds() as u64;
                if elapsed >= w.timeout_secs {
                    w.status = WatcherStatus::TimedOut;
                    expired.push(w.clone());
                }
            }
        }
        drop(watchers);
        if !expired.is_empty() {
            self.persist().await;
        }
        expired
    }

    /// Cancel a watcher by ID
    pub async fn cancel(&self, id: Uuid) -> bool {
        let cancelled = if let Some(w) = self.watchers.write().await.get_mut(&id) {
            if w.status == WatcherStatus::Active {
                w.status = WatcherStatus::Cancelled;
                true
            } else {
                false
            }
        } else {
            false
        };
        if cancelled {
            self.persist().await;
        }
        cancelled
    }

    /// Clean up completed/failed/timed-out watchers (older than 1 hour)
    pub async fn cleanup(&self) {
        let cutoff = Utc::now() - chrono::Duration::hours(1);
        let mut watchers = self.watchers.write().await;
        let before = watchers.len();
        watchers.retain(|_, w| w.status == WatcherStatus::Active || w.created_at > cutoff);
        let removed = before - watchers.len();
        drop(watchers);
        if removed > 0 {
            self.persist().await;
        }
    }
}
