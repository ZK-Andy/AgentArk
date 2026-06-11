//! Per-opportunity value ledger entry.
//!
//! `evolve_opportunities.ledger_json` remains the compact UI cache; this table
//! is the append-only-ish audit source for expected/measured/realized value.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "evolve_value_ledger")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub opportunity_id: String,
    pub phase: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub value_json: Json,
    #[sea_orm(nullable)]
    pub source_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
