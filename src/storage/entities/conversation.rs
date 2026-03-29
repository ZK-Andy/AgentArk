//! Conversation entity for chat persistence

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "conversations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub title: String,
    pub channel: String,
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i32,
    pub archived: bool,
    pub starred: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
