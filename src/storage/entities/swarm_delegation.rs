//! Swarm delegation entity for tracking delegated tasks

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "swarm_delegations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(nullable)]
    pub parent_task_id: Option<String>,
    pub agent_id: String,
    pub task_description: String,
    #[sea_orm(nullable)]
    pub result: Option<String>,
    #[sea_orm(column_type = "Integer")]
    pub success: i32,
    #[sea_orm(nullable)]
    pub confidence: Option<f32>,
    #[sea_orm(nullable)]
    pub execution_time_ms: Option<i32>,
    pub created_at: String,
    #[sea_orm(nullable)]
    pub completed_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
