//! Document entity for RAG document Q&A

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "documents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub project_id: Option<String>,
    pub chunk_count: i32,
    pub file_size: i64,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::document_chunk::Entity")]
    DocumentChunk,
}

impl Related<super::document_chunk::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DocumentChunk.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
