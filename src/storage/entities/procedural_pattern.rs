//! Learned procedural pattern entity for reusable workflow induction.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "procedural_patterns")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub intent_key: String,
    pub scope: String,
    #[sea_orm(nullable)]
    pub project_id: Option<String>,
    #[sea_orm(nullable)]
    pub conversation_id: Option<String>,
    pub title: String,
    pub trigger_summary: String,
    pub summary: String,
    #[sea_orm(nullable)]
    pub tool_sequence_digest: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub steps_json: Json,
    #[sea_orm(column_type = "JsonBinary")]
    pub tool_sequence_json: Json,
    pub sample_count: i32,
    pub success_count: i32,
    pub correction_count: i32,
    pub success_rate: f64,
    #[sea_orm(nullable)]
    pub last_validated_at: Option<String>,
    pub status: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub metadata: Json,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
