//! Database storage using SeaORM

pub mod entities;
pub mod encrypted;

use anyhow::Result;
#[allow(unused_imports)]
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryOrder, QuerySelect, Set, Schema, ConnectionTrait,
};
use std::path::Path;

pub use entities::*;

/// Database storage using SeaORM
#[derive(Clone)]
pub struct Storage {
    db: DatabaseConnection,
}

impl Storage {
    /// Create a new storage instance
    pub async fn new(data_dir: &Path) -> Result<Self> {
        let db_path = data_dir.join("cogniark.db");
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        let db = Database::connect(&db_url).await?;

        // Create tables if they don't exist
        Self::create_tables(&db).await?;

        Ok(Self { db })
    }

    /// Create all tables
    async fn create_tables(db: &DatabaseConnection) -> Result<()> {
        let backend = db.get_database_backend();
        let _schema = Schema::new(backend);

        // Create tables using raw SQL for SQLite compatibility
        db.execute_unprepared(
            r#"
            CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS episodes (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                context TEXT NOT NULL,
                embedding BLOB,
                timestamp TEXT NOT NULL,
                consolidated INTEGER DEFAULT 0,
                importance REAL DEFAULT 0.5,
                last_accessed TEXT,
                access_count INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS semantic_facts (
                id TEXT PRIMARY KEY,
                fact TEXT NOT NULL,
                confidence REAL NOT NULL,
                sources TEXT NOT NULL,
                embedding BLOB,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS actions (
                name TEXT PRIMARY KEY,
                version TEXT NOT NULL,
                wasm_hash TEXT,
                source TEXT NOT NULL,
                success_rate REAL DEFAULT 1.0,
                execution_count INTEGER DEFAULT 0,
                last_used TEXT
            );

            CREATE TABLE IF NOT EXISTS execution_proofs (
                id TEXT PRIMARY KEY,
                action_hash TEXT NOT NULL,
                input_hash TEXT NOT NULL,
                output_hash TEXT NOT NULL,
                prev_hash TEXT,
                timestamp TEXT NOT NULL,
                signature TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                action TEXT NOT NULL,
                arguments TEXT NOT NULL,
                approval TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                scheduled_for TEXT,
                cron TEXT,
                result TEXT,
                proof_id TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_episodes_timestamp ON episodes(timestamp);
            CREATE INDEX IF NOT EXISTS idx_proofs_timestamp ON execution_proofs(timestamp);
            "#,
        )
        .await?;

        // Run migrations for existing databases (add new columns if missing)
        Self::run_migrations(&db).await?;

        Ok(())
    }

    /// Run migrations for existing databases
    async fn run_migrations(db: &DatabaseConnection) -> Result<()> {
        // Add importance column if not exists
        let _ = db.execute_unprepared(
            "ALTER TABLE episodes ADD COLUMN importance REAL DEFAULT 0.5"
        ).await;

        // Add last_accessed column if not exists
        let _ = db.execute_unprepared(
            "ALTER TABLE episodes ADD COLUMN last_accessed TEXT"
        ).await;

        // Add access_count column if not exists
        let _ = db.execute_unprepared(
            "ALTER TABLE episodes ADD COLUMN access_count INTEGER DEFAULT 0"
        ).await;

        // Rename 'skill' column to 'action' in tasks table (migration from older schema)
        let _ = db.execute_unprepared(
            "ALTER TABLE tasks RENAME COLUMN skill TO action"
        ).await;

        // Rename 'skills' table to 'actions' if old table exists
        let _ = db.execute_unprepared(
            "ALTER TABLE skills RENAME TO actions"
        ).await;

        Ok(())
    }

    /// Get database connection
    #[allow(dead_code)]
    pub fn connection(&self) -> &DatabaseConnection {
        &self.db
    }

    // ==================== Key-Value Store ====================

    /// Get a value from the key-value store
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let result = kv_store::Entity::find_by_id(key.to_string())
            .one(&self.db)
            .await?;

        Ok(result.map(|m| m.value))
    }

    /// Set a value in the key-value store
    pub async fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        // Try to find existing
        let existing = kv_store::Entity::find_by_id(key.to_string())
            .one(&self.db)
            .await?;

        if existing.is_some() {
            // Update
            kv_store::ActiveModel {
                key: Set(key.to_string()),
                value: Set(value.to_vec()),
                created_at: sea_orm::NotSet,
                updated_at: Set(now),
            }
            .update(&self.db)
            .await?;
        } else {
            // Insert
            kv_store::ActiveModel {
                key: Set(key.to_string()),
                value: Set(value.to_vec()),
                created_at: Set(now.clone()),
                updated_at: Set(now),
            }
            .insert(&self.db)
            .await?;
        }

        Ok(())
    }

    /// Delete a key from the store
    #[allow(dead_code)]
    pub async fn delete(&self, key: &str) -> Result<()> {
        kv_store::Entity::delete_by_id(key.to_string())
            .exec(&self.db)
            .await?;
        Ok(())
    }

    // ==================== Episodes ====================

    /// Insert an episode with optional importance score
    #[allow(dead_code)]
    pub async fn insert_episode(
        &self,
        id: &str,
        content: &str,
        context: &str,
        embedding: Option<Vec<u8>>,
        timestamp: &str,
    ) -> Result<()> {
        self.insert_episode_with_importance(id, content, context, embedding, timestamp, 0.5).await
    }

    /// Insert an episode with explicit importance score
    pub async fn insert_episode_with_importance(
        &self,
        id: &str,
        content: &str,
        context: &str,
        embedding: Option<Vec<u8>>,
        timestamp: &str,
        importance: f32,
    ) -> Result<()> {
        episode::ActiveModel {
            id: Set(id.to_string()),
            content: Set(content.to_string()),
            context: Set(context.to_string()),
            embedding: Set(embedding),
            timestamp: Set(timestamp.to_string()),
            consolidated: Set(false),
            importance: Set(importance),
            last_accessed: Set(None),
            access_count: Set(0),
        }
        .insert(&self.db)
        .await?;

        Ok(())
    }

    /// Update episode access time (called when memory is retrieved)
    pub async fn touch_episode(&self, id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        // Use raw SQL to increment access_count atomically
        self.db.execute_unprepared(&format!(
            "UPDATE episodes SET last_accessed = '{}', access_count = access_count + 1 WHERE id = '{}'",
            now, id
        )).await?;

        Ok(())
    }

    /// Update episode importance score
    pub async fn update_episode_importance(&self, id: &str, importance: f32) -> Result<()> {
        self.db.execute_unprepared(&format!(
            "UPDATE episodes SET importance = {} WHERE id = '{}'",
            importance.clamp(0.0, 1.0), id
        )).await?;

        Ok(())
    }

    /// Get all episodes with their metadata for scoring
    pub async fn get_all_episodes_for_scoring(&self) -> Result<Vec<episode::Model>> {
        let episodes = episode::Entity::find()
            .order_by_desc(episode::Column::Timestamp)
            .all(&self.db)
            .await?;

        Ok(episodes)
    }

    /// Get all episodes (paginated)
    pub async fn get_episodes(&self, limit: u64, offset: u64) -> Result<Vec<episode::Model>> {
        let episodes = episode::Entity::find()
            .order_by_desc(episode::Column::Timestamp)
            .limit(limit)
            .offset(offset)
            .all(&self.db)
            .await?;

        Ok(episodes)
    }

    /// Count episodes
    pub async fn count_episodes(&self) -> Result<u64> {
        let count = episode::Entity::find().count(&self.db).await?;
        Ok(count)
    }

    // ==================== Semantic Facts ====================

    /// Insert a semantic fact
    pub async fn insert_fact(
        &self,
        id: &str,
        fact: &str,
        confidence: f32,
        sources: &str,
        embedding: Option<Vec<u8>>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        semantic_fact::ActiveModel {
            id: Set(id.to_string()),
            fact: Set(fact.to_string()),
            confidence: Set(confidence),
            sources: Set(sources.to_string()),
            embedding: Set(embedding),
            created_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        Ok(())
    }

    /// Get all semantic facts
    #[allow(dead_code)]
    pub async fn get_facts(&self) -> Result<Vec<semantic_fact::Model>> {
        let facts = semantic_fact::Entity::find().all(&self.db).await?;
        Ok(facts)
    }

    // ==================== Actions ====================

    /// Insert or update an action
    #[allow(dead_code)]
    pub async fn upsert_action(
        &self,
        name: &str,
        version: &str,
        wasm_hash: Option<&str>,
        source: &str,
    ) -> Result<()> {
        let existing = action::Entity::find_by_id(name.to_string())
            .one(&self.db)
            .await?;

        if existing.is_some() {
            action::ActiveModel {
                name: Set(name.to_string()),
                version: Set(version.to_string()),
                wasm_hash: Set(wasm_hash.map(|s| s.to_string())),
                source: Set(source.to_string()),
                ..Default::default()
            }
            .update(&self.db)
            .await?;
        } else {
            action::ActiveModel {
                name: Set(name.to_string()),
                version: Set(version.to_string()),
                wasm_hash: Set(wasm_hash.map(|s| s.to_string())),
                source: Set(source.to_string()),
                success_rate: Set(1.0),
                execution_count: Set(0),
                last_used: Set(None),
            }
            .insert(&self.db)
            .await?;
        }

        Ok(())
    }

    /// Record action execution
    #[allow(dead_code)]
    pub async fn record_action_execution(&self, name: &str, success: bool) -> Result<()> {
        if let Some(action_record) = action::Entity::find_by_id(name.to_string())
            .one(&self.db)
            .await?
        {
            let new_count = action_record.execution_count + 1;
            let new_success_rate = if success {
                (action_record.success_rate * action_record.execution_count as f32 + 1.0) / new_count as f32
            } else {
                (action_record.success_rate * action_record.execution_count as f32) / new_count as f32
            };

            action::ActiveModel {
                name: Set(name.to_string()),
                execution_count: Set(new_count),
                success_rate: Set(new_success_rate),
                last_used: Set(Some(chrono::Utc::now().to_rfc3339())),
                ..Default::default()
            }
            .update(&self.db)
            .await?;
        }

        Ok(())
    }

    // ==================== Execution Proofs ====================

    /// Insert an execution proof
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
        execution_proof::ActiveModel {
            id: Set(id.to_string()),
            action_hash: Set(action_hash.to_string()),
            input_hash: Set(input_hash.to_string()),
            output_hash: Set(output_hash.to_string()),
            prev_hash: Set(prev_hash.map(|s| s.to_string())),
            timestamp: Set(timestamp.to_string()),
            signature: Set(signature.to_string()),
        }
        .insert(&self.db)
        .await?;

        Ok(())
    }

    /// Get all execution proofs
    #[allow(dead_code)]
    pub async fn get_proofs(&self) -> Result<Vec<execution_proof::Model>> {
        let proofs = execution_proof::Entity::find()
            .order_by_asc(execution_proof::Column::Timestamp)
            .all(&self.db)
            .await?;

        Ok(proofs)
    }

    // ==================== Tasks ====================

    /// Insert a task
    pub async fn insert_task(&self, task: &crate::core::Task) -> Result<()> {
        task::ActiveModel {
            id: Set(task.id.to_string()),
            description: Set(task.description.clone()),
            action: Set(task.action.clone()),
            arguments: Set(serde_json::to_string(&task.arguments)?),
            approval: Set(serde_json::to_string(&task.approval)?),
            status: Set(serde_json::to_string(&task.status)?),
            created_at: Set(task.created_at.to_rfc3339()),
            scheduled_for: Set(task.scheduled_for.map(|t| t.to_rfc3339())),
            cron: Set(task.cron.clone()),
            result: Set(task.result.clone()),
            proof_id: Set(task.proof_id.map(|id| id.to_string())),
        }
        .insert(&self.db)
        .await?;

        Ok(())
    }

    /// Update task status
    pub async fn update_task_status(&self, id: &str, status: &str) -> Result<()> {
        task::ActiveModel {
            id: Set(id.to_string()),
            status: Set(status.to_string()),
            ..Default::default()
        }
        .update(&self.db)
        .await?;

        Ok(())
    }

    /// Update task fields
    pub async fn update_task(
        &self,
        id: &str,
        description: Option<String>,
        arguments: Option<String>,
        cron: Option<String>,
        scheduled_for: Option<String>,
    ) -> Result<()> {
        let mut model = task::ActiveModel {
            id: Set(id.to_string()),
            ..Default::default()
        };

        if let Some(desc) = description {
            model.description = Set(desc);
        }
        if let Some(args) = arguments {
            model.arguments = Set(args);
        }
        if cron.is_some() {
            model.cron = Set(cron);
        }
        if scheduled_for.is_some() {
            model.scheduled_for = Set(scheduled_for);
        }

        model.update(&self.db).await?;
        Ok(())
    }

    pub async fn update_task_status_and_result(
        &self,
        id: &str,
        status: &str,
        result: Option<&str>,
    ) -> Result<()> {
        let mut model = task::ActiveModel {
            id: Set(id.to_string()),
            status: Set(status.to_string()),
            ..Default::default()
        };
        if let Some(res) = result {
            model.result = Set(Some(res.to_string()));
        }
        model.update(&self.db).await?;
        Ok(())
    }

    /// Delete a task
    pub async fn delete_task(&self, id: &str) -> Result<()> {
        task::Entity::delete_by_id(id.to_string()).exec(&self.db).await?;
        Ok(())
    }

    /// Get all tasks
    pub async fn get_tasks(&self) -> Result<Vec<task::Model>> {
        let tasks = task::Entity::find().all(&self.db).await?;
        Ok(tasks)
    }
}
