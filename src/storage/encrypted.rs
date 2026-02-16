//! Encrypted storage wrapper
//!
//! Provides transparent encryption for sensitive data in the database.
//! Content fields (episode content, fact text, message content, KV values)
//! are encrypted with AES-256-GCM before storage and decrypted on retrieval.
//! Non-content fields (timestamps, IDs, metadata) remain in plaintext for querying.

use super::entities::{episode, semantic_fact};
use super::Storage;
use crate::crypto::KeyManager;
use anyhow::Result;
use std::sync::Arc;

/// Encrypted storage that wraps the base storage
/// and encrypts sensitive fields before storing
#[derive(Clone)]
pub struct EncryptedStorage {
    storage: Storage,
    key_manager: Arc<KeyManager>,
}

impl EncryptedStorage {
    /// Create a new encrypted storage
    pub fn new(storage: Storage, key_manager: Arc<KeyManager>) -> Self {
        Self {
            storage,
            key_manager,
        }
    }

    // ==================== Decrypt Helpers ====================

    /// Decrypt the content field of episodes, falling back to plaintext for legacy data
    fn decrypt_episode_content(&self, mut episodes: Vec<episode::Model>) -> Vec<episode::Model> {
        for ep in &mut episodes {
            if let Ok(decrypted) = self.key_manager.decrypt_string(&ep.content) {
                ep.content = decrypted;
            }
            // If decrypt fails, content is already plaintext (legacy) — leave as-is
        }
        episodes
    }

    /// Decrypt the fact field of semantic facts, falling back to plaintext for legacy data
    fn decrypt_fact_content(
        &self,
        mut facts: Vec<semantic_fact::Model>,
    ) -> Vec<semantic_fact::Model> {
        for f in &mut facts {
            if let Ok(decrypted) = self.key_manager.decrypt_string(&f.fact) {
                f.fact = decrypted;
            }
        }
        facts
    }

    // ==================== Encrypted Episodes ====================

    /// Insert an episode with encrypted content
    pub async fn insert_episode_encrypted(
        &self,
        id: &str,
        content: &str,
        context: &str,
        embedding: Option<Vec<u8>>,
        importance: f32,
        project_id: Option<&str>,
    ) -> Result<()> {
        let encrypted_content = self.key_manager.encrypt_string(content)?;
        self.storage
            .insert_episode(
                id,
                &encrypted_content,
                context,
                embedding,
                importance,
                project_id,
            )
            .await
    }

    /// Get all episodes for scoring and decrypt content
    pub async fn get_all_episodes_for_scoring_decrypted(&self) -> Result<Vec<episode::Model>> {
        let episodes = self.storage.get_all_episodes_for_scoring().await?;
        Ok(self.decrypt_episode_content(episodes))
    }

    /// Get all episodes for scoring by project and decrypt content
    pub async fn get_all_episodes_for_scoring_by_project_decrypted(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<episode::Model>> {
        let episodes = self
            .storage
            .get_all_episodes_for_scoring_by_project(project_id)
            .await?;
        Ok(self.decrypt_episode_content(episodes))
    }

    /// Get unconsolidated episodes and decrypt content
    pub async fn get_unconsolidated_episodes_decrypted(
        &self,
        limit: u64,
    ) -> Result<Vec<episode::Model>> {
        let episodes = self.storage.get_unconsolidated_episodes(limit).await?;
        Ok(self.decrypt_episode_content(episodes))
    }

    /// Get episodes by project and decrypt content
    pub async fn get_episodes_by_project_decrypted(
        &self,
        limit: u64,
        offset: u64,
        project_id: Option<&str>,
    ) -> Result<Vec<episode::Model>> {
        let episodes = self
            .storage
            .get_episodes_by_project(limit, offset, project_id)
            .await?;
        Ok(self.decrypt_episode_content(episodes))
    }

    // ==================== Encrypted Semantic Facts ====================

    /// Insert a semantic fact with encrypted content
    pub async fn insert_fact_encrypted(
        &self,
        id: &str,
        fact: &str,
        confidence: f32,
        sources: &str,
        embedding: Option<Vec<u8>>,
        project_id: Option<&str>,
    ) -> Result<()> {
        let encrypted_fact = self.key_manager.encrypt_string(fact)?;
        self.storage
            .insert_fact(
                id,
                &encrypted_fact,
                confidence,
                sources,
                embedding,
                project_id,
            )
            .await
    }

    /// Get facts and decrypt their content
    pub async fn get_facts_decrypted(&self) -> Result<Vec<semantic_fact::Model>> {
        let facts = self.storage.get_facts().await?;
        Ok(self.decrypt_fact_content(facts))
    }

    /// Get facts by project and decrypt their content (paginated)
    pub async fn get_facts_by_project_decrypted(
        &self,
        limit: u64,
        offset: u64,
        project_id: Option<&str>,
    ) -> Result<Vec<semantic_fact::Model>> {
        let facts = self
            .storage
            .get_facts_by_project(limit, offset, project_id)
            .await?;
        Ok(self.decrypt_fact_content(facts))
    }

    // ==================== Encrypted KV Store ====================

    /// Set an encrypted value in the KV store
    pub async fn set_encrypted(&self, key: &str, value: &[u8]) -> Result<()> {
        let encrypted = self.key_manager.encrypt(value)?;
        self.storage.set(key, &encrypted).await
    }

    /// Get and decrypt a value from the KV store
    pub async fn get_decrypted(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match self.storage.get(key).await? {
            Some(encrypted) => {
                match self.key_manager.decrypt(&encrypted) {
                    Ok(decrypted) => Ok(Some(decrypted)),
                    Err(_) => Ok(Some(encrypted)), // Legacy unencrypted data
                }
            }
            None => Ok(None),
        }
    }
}
