//! Watcher persistence entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "watchers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub payload: String,
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
