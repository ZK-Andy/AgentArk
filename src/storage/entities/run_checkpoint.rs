//! Execution run checkpoint entity.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "run_checkpoints")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i64,
    pub run_id: String,
    pub sequence_no: i32,
    pub stage: String,
    pub payload: String,
    pub created_at: String,
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
