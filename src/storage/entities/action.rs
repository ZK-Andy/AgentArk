//! Action entity for action registry

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "actions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub name: String,
    pub version: String,
    #[sea_orm(nullable)]
    pub wasm_hash: Option<String>,
    pub source: String,
    #[sea_orm(default_value = 1.0)]
    pub success_rate: f32,
    #[sea_orm(default_value = 0)]
    pub execution_count: i32,
    #[sea_orm(nullable)]
    pub last_used: Option<String>,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
