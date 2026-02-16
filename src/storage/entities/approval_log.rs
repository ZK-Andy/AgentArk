//! Approval log entity for persisting safety approval decisions

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "approval_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub action_name: String,
    pub arguments: String,
    pub rule_name: String,
    /// Status: "pending", "approved", "denied", "expired"
    pub status: String,
    pub requested_at: String,
    #[sea_orm(nullable)]
    pub resolved_at: Option<String>,
    /// Who resolved it: "user", "auto_timeout", "api"
    #[sea_orm(nullable)]
    pub resolved_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
