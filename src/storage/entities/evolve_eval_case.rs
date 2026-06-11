//! Usage-derived ArkEvolve eval case.
//!
//! Rows are keyed by stable source/opportunity identity so holdouts and
//! user-derived verdict cases survive process restarts and can be audited.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "evolve_eval_cases")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(nullable)]
    pub opportunity_id: Option<String>,
    pub case_kind: String,
    pub source_kind: String,
    pub source_ref: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub source_run_ids_json: Json,
    pub request_text: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub contract_event_json: Json,
    pub expected_behavior: String,
    pub disallowed_behavior: String,
    pub missing_info_policy: String,
    pub secret_policy: String,
    pub holdout: bool,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
