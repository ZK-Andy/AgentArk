//! Transactional execution with rollback support
//!
//! Based on arXiv:2512.12806 "Fault-Tolerant Sandboxing"
//!
//! Every potentially destructive operation is wrapped in a transaction.
//! If the operation fails, the system rolls back to the previous state.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// A filesystem transaction
#[allow(dead_code)]
#[derive(Debug)]
pub struct Transaction {
    /// Transaction ID
    pub id: Uuid,

    /// When the transaction started
    pub started_at: DateTime<Utc>,

    /// Snapshot directory
    pub snapshot_dir: PathBuf,

    /// Files that were modified
    pub modified_files: Vec<PathBuf>,

    /// Original file contents (for rollback)
    pub original_contents: Vec<(PathBuf, Option<Vec<u8>>)>,
}

impl Transaction {
    pub fn new(snapshot_dir: PathBuf) -> Self {
        Self {
            id: Uuid::new_v4(),
            started_at: Utc::now(),
            snapshot_dir,
            modified_files: vec![],
            original_contents: vec![],
        }
    }

    /// Record a file modification
    #[allow(dead_code)]
    pub fn record_modification(&mut self, path: &Path) -> Result<()> {
        // Store original content
        let original = if path.exists() {
            Some(std::fs::read(path)?)
        } else {
            None
        };

        self.original_contents.push((path.to_path_buf(), original));
        self.modified_files.push(path.to_path_buf());

        Ok(())
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
    #[allow(dead_code)]
    active_transactions: Vec<Transaction>,
}

impl TransactionManager {
    pub fn new(snapshot_dir: PathBuf) -> Self {
        Self {
            snapshot_dir,
            active_transactions: vec![],
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

    /// Create a full filesystem snapshot (for heavy operations)
    #[allow(dead_code)]
    pub async fn create_snapshot(&self, paths: &[PathBuf]) -> Result<Uuid> {
        let snapshot_id = Uuid::new_v4();
        let snapshot_path = self.snapshot_dir.join(snapshot_id.to_string());
        std::fs::create_dir_all(&snapshot_path)?;

        for path in paths {
            if path.exists() {
                let dest = snapshot_path.join(
                    path.file_name()
                        .ok_or_else(|| anyhow::anyhow!("Invalid path"))?,
                );

                // Try to use copy-on-write if available
                if let Err(_) = reflink_copy::reflink(path, &dest) {
                    // Fall back to regular copy
                    if path.is_dir() {
                        copy_dir_all(path, &dest)?;
                    } else {
                        std::fs::copy(path, &dest)?;
                    }
                }
            }
        }

        Ok(snapshot_id)
    }

    /// Restore from a snapshot
    #[allow(dead_code)]
    pub async fn restore_snapshot(&self, snapshot_id: Uuid, paths: &[PathBuf]) -> Result<()> {
        let snapshot_path = self.snapshot_dir.join(snapshot_id.to_string());

        if !snapshot_path.exists() {
            return Err(anyhow::anyhow!("Snapshot not found: {}", snapshot_id));
        }

        for path in paths {
            let src = snapshot_path.join(
                path.file_name()
                    .ok_or_else(|| anyhow::anyhow!("Invalid path"))?,
            );

            if src.exists() {
                if path.exists() {
                    if path.is_dir() {
                        std::fs::remove_dir_all(path)?;
                    } else {
                        std::fs::remove_file(path)?;
                    }
                }

                if src.is_dir() {
                    copy_dir_all(&src, path)?;
                } else {
                    std::fs::copy(&src, path)?;
                }
            }
        }

        // Clean up snapshot
        std::fs::remove_dir_all(snapshot_path)?;

        Ok(())
    }
}

/// Recursively copy a directory
#[allow(dead_code)]
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
