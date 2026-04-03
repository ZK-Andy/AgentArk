//! Tool attempt persistence entity.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tool_attempts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub run_id: String,
    pub sequence_no: i32,
    pub tool_name: String,
    pub status: String,
    #[sea_orm(nullable)]
    pub failure_class: Option<String>,
    pub retryable: bool,
    pub side_effect_level: String,
    #[sea_orm(nullable)]
    pub idempotency_key: Option<String>,
    pub arguments_json: String,
    pub output_json: String,
    pub started_at: String,
    #[sea_orm(nullable)]
    pub completed_at: Option<String>,
    #[sea_orm(nullable)]
    pub error_text: Option<String>,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::execution_run::Entity",
        from = "Column::RunId",
        to = "super::execution_run::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    ExecutionRun,
}

impl Related<super::execution_run::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ExecutionRun.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
