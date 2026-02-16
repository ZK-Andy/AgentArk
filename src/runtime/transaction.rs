//! Transactional execution with rollback support
//!
//! Based on arXiv:2512.12806 "Fault-Tolerant Sandboxing"
//!
//! Every potentially destructive operation is wrapped in a transaction.
//! If the operation fails, the system rolls back to the previous state.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use uuid::Uuid;

/// A filesystem transaction
#[derive(Debug)]
pub struct Transaction {
    /// Transaction ID
    pub id: Uuid,

    /// When the transaction started
    pub _started_at: DateTime<Utc>,

    /// Snapshot directory
    pub _snapshot_dir: PathBuf,

    /// Files that were modified
    pub _modified_files: Vec<PathBuf>,

    /// Original file contents (for rollback)
    pub original_contents: Vec<(PathBuf, Option<Vec<u8>>)>,
}

impl Transaction {
    pub fn new(snapshot_dir: PathBuf) -> Self {
        Self {
            id: Uuid::new_v4(),
            _started_at: Utc::now(),
            _snapshot_dir: snapshot_dir,
            _modified_files: vec![],
            original_contents: vec![],
        }
    }

    /// Rollback all changes
    pub fn rollback(&self) -> Result<()> {
        for (path, original) in &self.original_contents {
            match original {
                Some(content) => {
                    // Restore original content
                    std::fs::write(path, content)?;
                }
                None => {
                    // File didn't exist before - remove it
                    if path.exists() {
                        std::fs::remove_file(path)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Manages transactions
pub struct TransactionManager {
    snapshot_dir: PathBuf,
    _active_transactions: Vec<Transaction>,
}

impl TransactionManager {
    pub fn new(snapshot_dir: PathBuf) -> Self {
        Self {
            snapshot_dir,
            _active_transactions: vec![],
        }
    }

    /// Begin a new transaction
    pub async fn begin(&mut self) -> Result<Transaction> {
        let tx = Transaction::new(self.snapshot_dir.clone());
        tracing::debug!("Started transaction {}", tx.id);
        Ok(tx)
    }

    /// Commit a transaction (just cleanup)
    pub async fn commit(&mut self, tx: Transaction) -> Result<()> {
        tracing::debug!("Committed transaction {}", tx.id);
        // Clean up any snapshots
        let snapshot_path = self.snapshot_dir.join(tx.id.to_string());
        if snapshot_path.exists() {
            std::fs::remove_dir_all(snapshot_path)?;
        }
        Ok(())
    }

    /// Rollback a transaction
    pub async fn rollback(&mut self, tx: Transaction) -> Result<()> {
        tracing::warn!("Rolling back transaction {}", tx.id);
        tx.rollback()?;

        // Clean up snapshot
        let snapshot_path = self.snapshot_dir.join(tx.id.to_string());
        if snapshot_path.exists() {
            std::fs::remove_dir_all(snapshot_path)?;
        }

        Ok(())
    }
}
