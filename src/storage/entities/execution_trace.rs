//! Persisted execution trace entity for Trace history and detail views.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "execution_traces")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub message: String,
    pub channel: String,
    #[sea_orm(nullable)]
    pub started_at: Option<String>,
    #[sea_orm(nullable)]
    pub completed_at: Option<String>,
    #[sea_orm(nullable)]
    pub duration_ms: Option<i32>,
    pub step_count: i32,
    pub steps_json: String,
    #[sea_orm(nullable)]
    pub response: Option<String>,
    #[sea_orm(nullable)]
    pub proof_id: Option<String>,
    #[sea_orm(nullable)]
    pub model: Option<String>,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub total_tokens: i32,
    pub cost_usd: f64,
    #[sea_orm(nullable)]
    pub complexity: Option<String>,
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
