//! Encrypted storage wrapper
//!
//! Provides transparent encryption for sensitive data in the database.

use crate::crypto::KeyManager;
use super::Storage;
use anyhow::Result;
use std::sync::Arc;

/// Encrypted storage that wraps the base storage
/// and encrypts sensitive fields before storing
#[allow(dead_code)]
#[derive(Clone)]
pub struct EncryptedStorage {
    storage: Storage,
    key_manager: Arc<KeyManager>,
}

impl EncryptedStorage {
    /// Create a new encrypted storage
    #[allow(dead_code)]
    pub fn new(storage: Storage, key_manager: Arc<KeyManager>) -> Self {
        Self { storage, key_manager }
    }

    /// Get the underlying storage (for non-encrypted operations)
    #[allow(dead_code)]
    pub fn inner(&self) -> &Storage {
        &self.storage
    }

    /// Get the key manager
    #[allow(dead_code)]
    pub fn key_manager(&self) -> &KeyManager {
        &self.key_manager
    }

    // ==================== Encrypted Episodes ====================

    /// Insert an episode with encrypted content
    #[allow(dead_code)]
    pub async fn insert_episode_encrypted(
        &self,
        id: &str,
        content: &str,
        context: &str,
        embedding: Option<Vec<u8>>,
        timestamp: &str,
    ) -> Result<()> {
        // Encrypt content (conversation memory is sensitive)
        let encrypted_content = self.key_manager.encrypt_string(content)?;

        self.storage.insert_episode(
            id,
            &encrypted_content,
            context, // Context can remain unencrypted for search
            embedding,
            timestamp,
        ).await
    }

    /// Get episodes and decrypt their content
    #[allow(dead_code)]
    pub async fn get_episodes_decrypted(&self, limit: u64, offset: u64) -> Result<Vec<DecryptedEpisode>> {
        let episodes = self.storage.get_episodes(limit, offset).await?;

        let mut decrypted = Vec::with_capacity(episodes.len());
        for ep in episodes {
            let content = match self.key_manager.decrypt_string(&ep.content) {
                Ok(c) => c,
                Err(_) => {
                    // If decryption fails, content might be unencrypted (legacy)
                    ep.content.clone()
                }
            };

            decrypted.push(DecryptedEpisode {
                id: ep.id,
                content,
                context: ep.context,
                embedding: ep.embedding,
                timestamp: ep.timestamp,
                consolidated: ep.consolidated,
            });
        }

        Ok(decrypted)
    }

    // ==================== Encrypted Semantic Facts ====================

    /// Insert a semantic fact with encrypted content
    #[allow(dead_code)]
    pub async fn insert_fact_encrypted(
        &self,
        id: &str,
        fact: &str,
        confidence: f32,
        sources: &str,
        embedding: Option<Vec<u8>>,
    ) -> Result<()> {
        let encrypted_fact = self.key_manager.encrypt_string(fact)?;

        self.storage.insert_fact(
            id,
            &encrypted_fact,
            confidence,
            sources,
            embedding,
        ).await
    }

    /// Get facts and decrypt their content
    #[allow(dead_code)]
    pub async fn get_facts_decrypted(&self) -> Result<Vec<DecryptedFact>> {
        let facts = self.storage.get_facts().await?;

        let mut decrypted = Vec::with_capacity(facts.len());
        for f in facts {
            let fact = match self.key_manager.decrypt_string(&f.fact) {
                Ok(c) => c,
                Err(_) => f.fact.clone(), // Legacy unencrypted
            };

            decrypted.push(DecryptedFact {
                id: f.id,
                fact,
                confidence: f.confidence,
                sources: f.sources,
                embedding: f.embedding,
                created_at: f.created_at,
            });
        }

        Ok(decrypted)
    }

    // ==================== Encrypted KV Store ====================

    /// Set an encrypted value in the KV store
    #[allow(dead_code)]
    pub async fn set_encrypted(&self, key: &str, value: &[u8]) -> Result<()> {
        let encrypted = self.key_manager.encrypt(value)?;
        self.storage.set(key, &encrypted).await
    }

    /// Get and decrypt a value from the KV store
    #[allow(dead_code)]
    pub async fn get_decrypted(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match self.storage.get(key).await? {
            Some(encrypted) => {
                match self.key_manager.decrypt(&encrypted) {
                    Ok(decrypted) => Ok(Some(decrypted)),
                    Err(_) => {
                        // Might be legacy unencrypted data
                        Ok(Some(encrypted))
                    }
                }
            }
            None => Ok(None),
        }
    }

    // ==================== Pass-through methods ====================

    #[allow(dead_code)]
    pub async fn count_episodes(&self) -> Result<u64> {
        self.storage.count_episodes().await
    }

    #[allow(dead_code)]
    pub async fn upsert_action(
        &self,
        name: &str,
        version: &str,
        wasm_hash: Option<&str>,
        source: &str,
    ) -> Result<()> {
        self.storage.upsert_action(name, version, wasm_hash, source).await
    }

    #[allow(dead_code)]
    pub async fn record_action_execution(&self, name: &str, success: bool) -> Result<()> {
        self.storage.record_action_execution(name, success).await
    }

    #[allow(dead_code)]
    pub async fn insert_proof(
        &self,
        id: &str,
        action_hash: &str,
        input_hash: &str,
        output_hash: &str,
        prev_hash: Option<&str>,
        timestamp: &str,
        signature: &str,
    ) -> Result<()> {
        self.storage.insert_proof(id, action_hash, input_hash, output_hash, prev_hash, timestamp, signature).await
    }

    #[allow(dead_code)]
    pub async fn get_proofs(&self) -> Result<Vec<super::execution_proof::Model>> {
        self.storage.get_proofs().await
    }

    #[allow(dead_code)]
    pub async fn insert_task(&self, task: &crate::core::Task) -> Result<()> {
        self.storage.insert_task(task).await
    }

    #[allow(dead_code)]
    pub async fn update_task_status(&self, id: &str, status: &str) -> Result<()> {
        self.storage.update_task_status(id, status).await
    }

    #[allow(dead_code)]
    pub async fn get_tasks(&self) -> Result<Vec<super::task::Model>> {
        self.storage.get_tasks().await
    }
}

/// Decrypted episode (after retrieval)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DecryptedEpisode {
    pub id: String,
    pub content: String,
    pub context: String,
    pub embedding: Option<Vec<u8>>,
    pub timestamp: String,
    pub consolidated: bool,
}

/// Decrypted semantic fact (after retrieval)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DecryptedFact {
    pub id: String,
    pub fact: String,
    pub confidence: f32,
    pub sources: String,
    pub embedding: Option<Vec<u8>>,
    pub created_at: String,
}
