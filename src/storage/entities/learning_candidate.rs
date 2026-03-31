//! Approval-gated learning candidate entity.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "learning_candidates")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub candidate_type: String,
    pub subject_key: String,
    pub title: String,
    #[sea_orm(nullable)]
    pub summary: Option<String>,
    #[sea_orm(nullable)]
    pub project_id: Option<String>,
    #[sea_orm(nullable)]
    pub conversation_id: Option<String>,
    #[sea_orm(nullable)]
    pub pattern_id: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub evidence_refs: Json,
    #[sea_orm(column_type = "JsonBinary")]
    pub proposed_content: Json,
    pub confidence: f64,
    pub approval_status: String,
    #[sea_orm(nullable)]
    pub review_notes: Option<String>,
    #[sea_orm(nullable)]
    pub reviewed_at: Option<String>,
    #[sea_orm(nullable)]
    pub approved_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
