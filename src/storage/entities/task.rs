//! Task entity for task queue

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub description: String,
    pub action: String,
    pub arguments: String,
    pub approval: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[sea_orm(nullable)]
    pub scheduled_for: Option<String>,
    #[sea_orm(nullable)]
    pub cron: Option<String>,
    #[sea_orm(nullable)]
    pub result: Option<String>,
    #[sea_orm(nullable)]
    pub proof_id: Option<String>,
    #[sea_orm(nullable)]
    pub priority: Option<f64>,
    #[sea_orm(nullable)]
    pub urgency: Option<f64>,
    #[sea_orm(nullable)]
    pub importance: Option<f64>,
    #[sea_orm(nullable)]
    pub eisenhower_quadrant: Option<i32>,
    #[sea_orm(nullable)]
    pub lease_owner: Option<String>,
    #[sea_orm(nullable)]
    pub lease_expires_at: Option<String>,
    pub lease_version: i32,
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
