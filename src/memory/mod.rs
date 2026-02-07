//! Cognitive Memory System - Episodic, Semantic, and Procedural Memory
//!
//! Inspired by human memory systems and recent research:
//! - arXiv:2512.13564 "Memory in the Age of AI Agents"
//! - arXiv:2601.01885 "Agentic Memory (AgeMem)"
//! - Park et al. "Generative Agents" (2023) - Memory decay and retrieval scoring

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use crate::storage::Storage;

/// Memory decay configuration
/// Based on Generative Agents: final_score = α*relevance + β*recency + γ*importance
#[derive(Debug, Clone)]
pub struct MemoryDecayConfig {
    /// Weight for relevance/similarity score (α)
    pub relevance_weight: f32,
    /// Weight for recency score (β)
    pub recency_weight: f32,
    /// Weight for importance score (γ)
    pub importance_weight: f32,
    /// Decay rate (λ) - higher = faster decay
    /// recency = exp(-λ * hours_since_creation)
    pub decay_rate: f32,
    /// Bonus for recently accessed memories
    pub access_recency_bonus: f32,
}

impl Default for MemoryDecayConfig {
    fn default() -> Self {
        Self {
            relevance_weight: 1.0,
            recency_weight: 1.0,
            importance_weight: 1.0,
            decay_rate: 0.995,  // ~50% decay per day (24 hours)
            access_recency_bonus: 0.1,
        }
    }
}

/// Context for an episodic memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeContext {
    pub channel: String,
    pub timestamp: DateTime<Utc>,
    pub location: Option<String>,
    pub participants: Vec<String>,
}

/// A memory entry (can be episodic or semantic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: Uuid,
    pub content: String,
    pub memory_type: MemoryType,
    pub timestamp: DateTime<Utc>,
    /// Semantic similarity to current query (0.0-1.0)
    pub relevance_score: f32,
    /// User/LLM-assigned importance (0.0-1.0)
    pub importance: f32,
    /// Time-decayed recency score (0.0-1.0)
    pub recency_score: f32,
    /// Final combined score used for ranking
    pub final_score: f32,
    /// Number of times this memory was accessed
    pub access_count: i32,
}

/// Type of memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryType {
    /// Specific experiences with context
    Episodic { context: EpisodeContext },
    /// Generalized facts/knowledge
    Semantic { confidence: f32, sources: Vec<Uuid> },
    /// Learned actions/procedures
    Procedural { action_name: String, success_rate: f32 },
}

/// Cognitive memory system managing all memory types
pub struct CognitiveMemory {
    storage: Arc<Storage>,
    _embedding_model: String,
    episode_count: usize,
    /// Configuration for memory decay and scoring
    decay_config: MemoryDecayConfig,
}

impl CognitiveMemory {
    pub async fn new(_data_dir: &Path, storage: Storage) -> Result<Self> {
        Self::with_config(_data_dir, storage, MemoryDecayConfig::default()).await
    }

    pub async fn with_config(_data_dir: &Path, storage: Storage, decay_config: MemoryDecayConfig) -> Result<Self> {
        let storage = Arc::new(storage);

        // Count existing episodes
        let episode_count = storage.count_episodes().await.unwrap_or(0) as usize;

        Ok(Self {
            storage,
            _embedding_model: "BAAI/bge-small-en-v1.5".to_string(),
            episode_count,
            decay_config,
        })
    }

    /// Calculate recency score using exponential decay
    /// recency = exp(-λ * hours_since_creation)
    fn calculate_recency_score(&self, timestamp: DateTime<Utc>, last_accessed: Option<DateTime<Utc>>) -> f32 {
        let now = Utc::now();
        let hours_since_creation = (now - timestamp).num_hours() as f32;

        // Base recency from creation time
        let base_recency = (-self.decay_config.decay_rate * hours_since_creation / 24.0).exp();

        // Bonus if recently accessed
        let access_bonus = if let Some(last_access) = last_accessed {
            let hours_since_access = (now - last_access).num_hours() as f32;
            let access_recency = (-self.decay_config.decay_rate * hours_since_access / 24.0).exp();
            access_recency * self.decay_config.access_recency_bonus
        } else {
            0.0
        };

        (base_recency + access_bonus).min(1.0)
    }

    /// Calculate final memory score using weighted combination
    /// final_score = α*relevance + β*recency + γ*importance
    fn calculate_final_score(&self, relevance: f32, recency: f32, importance: f32) -> f32 {
        let config = &self.decay_config;

        // Normalize weights
        let total_weight = config.relevance_weight + config.recency_weight + config.importance_weight;

        if total_weight == 0.0 {
            return 0.0;
        }

        let normalized_relevance = config.relevance_weight / total_weight;
        let normalized_recency = config.recency_weight / total_weight;
        let normalized_importance = config.importance_weight / total_weight;

        normalized_relevance * relevance
            + normalized_recency * recency
            + normalized_importance * importance
    }

    /// Calculate simple relevance score based on word overlap
    fn calculate_relevance(&self, query: &str, content: &str) -> f32 {
        let query_lower = query.to_lowercase();
        let content_lower = content.to_lowercase();

        let query_words: std::collections::HashSet<&str> = query_lower
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();

        let content_words: std::collections::HashSet<&str> = content_lower
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();

        if query_words.is_empty() || content_words.is_empty() {
            return 0.0;
        }

        let intersection = query_words.intersection(&content_words).count();
        let query_coverage = intersection as f32 / query_words.len() as f32;

        // Boost for exact phrase matches
        let phrase_boost = if content_lower.contains(&query_lower) { 0.3 } else { 0.0 };

        (query_coverage + phrase_boost).min(1.0)
    }

    /// Add an episodic memory with default importance (0.5)
    pub async fn add_episode(&mut self, content: String, context: EpisodeContext) -> Result<Uuid> {
        self.add_episode_with_importance(content, context, 0.5).await
    }

    /// Add an episodic memory with explicit importance score
    /// importance: 0.0 (trivial) to 1.0 (critical)
    pub async fn add_episode_with_importance(
        &mut self,
        content: String,
        context: EpisodeContext,
        importance: f32,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let timestamp = context.timestamp;

        // Generate embedding for the content (placeholder - would use fastembed in production)
        let embedding: Option<Vec<u8>> = None;

        // Store in database using ORM
        let context_json = serde_json::to_string(&context)?;
        self.storage
            .insert_episode_with_importance(
                &id.to_string(),
                &content,
                &context_json,
                embedding,
                &timestamp.to_rfc3339(),
                importance.clamp(0.0, 1.0),
            )
            .await?;

        self.episode_count += 1;

        // Trigger consolidation if needed
        if self.episode_count % 100 == 0 {
            self.maybe_consolidate().await?;
        }

        Ok(id)
    }

    /// Add a semantic fact
    pub async fn add_fact(
        &mut self,
        fact: String,
        confidence: f32,
        sources: Vec<Uuid>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let embedding: Option<Vec<u8>> = None;
        let sources_json = serde_json::to_string(&sources)?;

        self.storage
            .insert_fact(
                &id.to_string(),
                &fact,
                confidence,
                &sources_json,
                embedding,
            )
            .await?;

        Ok(id)
    }

    /// Retrieve relevant memories for a query using decay-based scoring
    /// Implements: final_score = α*relevance + β*recency + γ*importance
    pub async fn retrieve_relevant(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        // Get all episodes for scoring (we need to score them all, then take top N)
        let episodes = self.storage.get_all_episodes_for_scoring().await?;

        let mut entries: Vec<MemoryEntry> = episodes
            .into_iter()
            .map(|e| {
                let context: EpisodeContext =
                    serde_json::from_str(&e.context).unwrap_or(EpisodeContext {
                        channel: "unknown".to_string(),
                        timestamp: Utc::now(),
                        location: None,
                        participants: vec![],
                    });

                let timestamp = chrono::DateTime::parse_from_rfc3339(&e.timestamp)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                let last_accessed = e.last_accessed.as_ref().and_then(|la| {
                    chrono::DateTime::parse_from_rfc3339(la)
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok()
                });

                // Calculate scores
                let relevance_score = self.calculate_relevance(query, &e.content);
                let recency_score = self.calculate_recency_score(timestamp, last_accessed);
                let importance = e.importance;

                // Calculate final weighted score
                let final_score = self.calculate_final_score(relevance_score, recency_score, importance);

                MemoryEntry {
                    id: Uuid::parse_str(&e.id).unwrap_or_else(|_| Uuid::new_v4()),
                    content: e.content,
                    memory_type: MemoryType::Episodic { context },
                    timestamp,
                    relevance_score,
                    importance,
                    recency_score,
                    final_score,
                    access_count: e.access_count,
                }
            })
            .collect();

        // Sort by final score (highest first)
        entries.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap_or(std::cmp::Ordering::Equal));

        // Take top N and update their access times
        let top_entries: Vec<MemoryEntry> = entries.into_iter().take(limit).collect();

        // Update access times for retrieved memories (async, fire-and-forget)
        for entry in &top_entries {
            let _ = self.storage.touch_episode(&entry.id.to_string()).await;
        }

        Ok(top_entries)
    }

    /// Retrieve memories without updating access times (for inspection/debugging)
    #[allow(dead_code)]
    pub async fn peek_relevant(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        let episodes = self.storage.get_all_episodes_for_scoring().await?;

        let mut entries: Vec<MemoryEntry> = episodes
            .into_iter()
            .map(|e| {
                let context: EpisodeContext =
                    serde_json::from_str(&e.context).unwrap_or(EpisodeContext {
                        channel: "unknown".to_string(),
                        timestamp: Utc::now(),
                        location: None,
                        participants: vec![],
                    });

                let timestamp = chrono::DateTime::parse_from_rfc3339(&e.timestamp)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                let last_accessed = e.last_accessed.as_ref().and_then(|la| {
                    chrono::DateTime::parse_from_rfc3339(la)
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok()
                });

                let relevance_score = self.calculate_relevance(query, &e.content);
                let recency_score = self.calculate_recency_score(timestamp, last_accessed);
                let importance = e.importance;
                let final_score = self.calculate_final_score(relevance_score, recency_score, importance);

                MemoryEntry {
                    id: Uuid::parse_str(&e.id).unwrap_or_else(|_| Uuid::new_v4()),
                    content: e.content,
                    memory_type: MemoryType::Episodic { context },
                    timestamp,
                    relevance_score,
                    importance,
                    recency_score,
                    final_score,
                    access_count: e.access_count,
                }
            })
            .collect();

        entries.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(entries.into_iter().take(limit).collect())
    }

    /// Update the importance of a specific memory
    #[allow(dead_code)]
    pub async fn set_importance(&self, memory_id: Uuid, importance: f32) -> Result<()> {
        self.storage.update_episode_importance(&memory_id.to_string(), importance).await
    }

    /// Get decay configuration
    #[allow(dead_code)]
    pub fn decay_config(&self) -> &MemoryDecayConfig {
        &self.decay_config
    }

    /// Update decay configuration
    #[allow(dead_code)]
    pub fn set_decay_config(&mut self, config: MemoryDecayConfig) {
        self.decay_config = config;
    }

    /// Consolidate episodic memories into semantic knowledge
    /// This extracts general facts from specific episodes
    async fn maybe_consolidate(&mut self) -> Result<()> {
        tracing::debug!("Starting memory consolidation...");

        // Get unconsolidated episodes (limit to 50 at a time)
        let episodes = self.storage.get_episodes(50, 0).await?;

        if episodes.is_empty() {
            tracing::debug!("No episodes to consolidate");
            return Ok(());
        }

        // Group episodes by similar content for fact extraction
        let mut content_groups: Vec<Vec<String>> = Vec::new();

        for episode in &episodes {
            let mut found_group = false;
            for group in &mut content_groups {
                if !group.is_empty() && self.content_similarity(&group[0], &episode.content) > 0.3 {
                    group.push(episode.content.clone());
                    found_group = true;
                    break;
                }
            }
            if !found_group {
                content_groups.push(vec![episode.content.clone()]);
            }
        }

        // Extract facts from groups with multiple similar episodes
        for group in content_groups {
            if group.len() >= 2 {
                // Multiple similar episodes suggest a recurring pattern/fact
                let fact = self.extract_fact_from_group(&group);
                if let Some(fact_text) = fact {
                    // Store as semantic fact with high confidence due to repetition
                    let episode_ids: Vec<Uuid> = episodes
                        .iter()
                        .filter(|e| group.contains(&e.content))
                        .filter_map(|e| Uuid::parse_str(&e.id).ok())
                        .collect();

                    let confidence = (group.len() as f32 / 10.0).min(0.95);
                    self.add_fact(fact_text, confidence, episode_ids).await?;
                    tracing::info!("Consolidated {} episodes into semantic fact", group.len());
                }
            }
        }

        tracing::debug!("Memory consolidation complete");
        Ok(())
    }

    /// Calculate simple content similarity using word overlap
    fn content_similarity(&self, a: &str, b: &str) -> f32 {
        let a_lower = a.to_lowercase();
        let b_lower = b.to_lowercase();

        let words_a: std::collections::HashSet<&str> = a_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();
        let words_b: std::collections::HashSet<&str> = b_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();

        if words_a.is_empty() || words_b.is_empty() {
            return 0.0;
        }

        let intersection = words_a.intersection(&words_b).count();
        let union = words_a.union(&words_b).count();

        intersection as f32 / union as f32
    }

    /// Extract a general fact from a group of similar episodes
    fn extract_fact_from_group(&self, group: &[String]) -> Option<String> {
        if group.is_empty() {
            return None;
        }

        // Find common phrases/patterns across episodes
        let first = &group[0];
        let words: Vec<&str> = first.split_whitespace().collect();

        if words.len() < 3 {
            return None;
        }

        // Try to find the longest common substring pattern
        let mut best_pattern = String::new();

        for window_size in (3..=words.len().min(10)).rev() {
            for window in words.windows(window_size) {
                let pattern: String = window.join(" ");
                let pattern_lower = pattern.to_lowercase();

                // Check if pattern appears in most episodes
                let matches = group.iter()
                    .filter(|e| e.to_lowercase().contains(&pattern_lower))
                    .count();

                if matches >= group.len() / 2 + 1 {
                    best_pattern = pattern;
                    break;
                }
            }
            if !best_pattern.is_empty() {
                break;
            }
        }

        if best_pattern.len() >= 10 {
            Some(format!("Recurring pattern: {}", best_pattern))
        } else {
            // Fall back to summary of first episode
            let summary: String = first.chars().take(100).collect();
            Some(format!("Observed: {}...", summary))
        }
    }

    /// Get total entry count
    pub fn entry_count(&self) -> usize {
        self.episode_count
    }
}
