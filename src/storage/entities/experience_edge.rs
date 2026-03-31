//! Typed relationship edges for the Postgres-native experience graph.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "experience_edges")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub source_ref: String,
    pub source_kind: String,
    pub target_ref: String,
    pub target_kind: String,
    pub edge_type: String,
    pub weight: f64,
    #[sea_orm(nullable)]
    pub source_run_id: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub metadata: Json,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
