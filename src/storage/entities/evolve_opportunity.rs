//! Persisted ArkEvolve opportunity: a usage-mined, value-vetted optimization
//! candidate with a stable identity and a full lifecycle (mined → vetted →
//! surfaced → approved → testing → deployed/dismissed/reverted). Segments are
//! free-text semantic descriptors — never fixed category enums.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "evolve_opportunities")]
pub struct Model {
    /// Stable content hash of (miner_key, segment_key, target_surface).
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// Internal detector id (e.g. "token_hotspot") — never user-facing copy.
    pub miner_key: String,
    /// Lifecycle: mined | vetted | surfaced | approved | testing | deployed
    /// | dismissed | reverted | rejected.
    pub status: String,
    /// Human-facing title (written by the value-verdict pass).
    pub title: String,
    /// Human-facing rationale (written by the value-verdict pass).
    pub description: String,
    /// Free-text semantic segment descriptor ("your data-analysis sessions").
    pub segment_label: String,
    /// Stable machine key for the segment (content-derived, not positional).
    pub segment_key: String,
    /// Optimization target surface (prompt bundle / fragment / strategy).
    pub target_surface: String,
    /// Evidence: run ids, sample counts, aggregate metrics.
    #[sea_orm(column_type = "JsonBinary")]
    pub evidence_json: Json,
    /// Multidimensional expected benefit with confidence.
    #[sea_orm(column_type = "JsonBinary")]
    pub expected_benefit_json: Json,
    /// Risk descriptor: blast radius, surface touched.
    #[sea_orm(column_type = "JsonBinary")]
    pub risk_json: Json,
    /// Run ids reserved as holdout eval cases for the promotion gate.
    #[sea_orm(column_type = "JsonBinary")]
    pub holdout_run_ids_json: Json,
    /// LLM value verdict {useful, reason, judged_at}.
    #[sea_orm(column_type = "JsonBinary")]
    pub verdict_json: Json,
    /// Value ledger: expected at approval, measured during testing, realized
    /// at deploy/revert. Filled by the canary judge.
    #[sea_orm(column_type = "JsonBinary")]
    pub ledger_json: Json,
    #[sea_orm(nullable)]
    pub gepa_job_id: Option<String>,
    #[sea_orm(nullable)]
    pub decided_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
