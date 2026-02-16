//! Episode entity for episodic memory

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "episodes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub content: String,
    pub context: String,
    #[sea_orm(column_type = "Blob", nullable)]
    pub embedding: Option<Vec<u8>>,
    pub timestamp: String,
    #[sea_orm(default_value = false)]
    pub consolidated: bool,
    /// Importance score (0.0-1.0) - can be set manually or by LLM
    #[sea_orm(default_value = "0.5")]
    pub importance: f32,
    /// Last time this memory was accessed/retrieved
    #[sea_orm(nullable)]
    pub last_accessed: Option<String>,
    /// Number of times this memory has been accessed
    #[sea_orm(default_value = "0")]
    pub access_count: i32,
    /// Optional project scope
    #[sea_orm(nullable)]
    pub project_id: Option<String>,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
