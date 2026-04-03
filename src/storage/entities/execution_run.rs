//! Execution run persistence entity.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "execution_runs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub kind: String,
    #[sea_orm(nullable)]
    pub request_id: Option<String>,
    pub status: String,
    pub current_stage: String,
    #[sea_orm(nullable)]
    pub lease_owner: Option<String>,
    #[sea_orm(nullable)]
    pub lease_expires_at: Option<String>,
    pub attempt: i32,
    #[sea_orm(nullable)]
    pub deadline_at: Option<String>,
    pub cancellation_requested: bool,
    pub degradation: String,
    #[sea_orm(nullable)]
    pub last_error: Option<String>,
    #[sea_orm(nullable)]
    pub result_summary: Option<String>,
    #[sea_orm(nullable)]
    pub trace_id: Option<String>,
    #[sea_orm(nullable)]
    pub conversation_id: Option<String>,
    #[sea_orm(nullable)]
    pub channel: Option<String>,
    #[sea_orm(nullable)]
    pub request_message: Option<String>,
    pub attempted_models: String,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::run_checkpoint::Entity")]
    RunCheckpoint,
    #[sea_orm(has_many = "super::tool_attempt::Entity")]
    ToolAttempt,
}

impl Related<super::run_checkpoint::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RunCheckpoint.def()
    }
}

impl Related<super::tool_attempt::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ToolAttempt.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
