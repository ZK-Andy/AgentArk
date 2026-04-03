//! Automation supervisor state persistence entity.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "automation_supervisor_states")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub automation_id: String,
    pub updated_at: String,
    pub payload: String,
    #[sea_orm(nullable)]
    pub next_retry_at: Option<String>,
    #[sea_orm(nullable)]
    pub last_run_id: Option<String>,
    pub consecutive_failures: i32,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
