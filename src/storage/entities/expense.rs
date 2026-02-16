//! Expense tracking entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "expenses")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub amount: f64,
    pub currency: String,
    pub category: String,
    pub description: String,
    pub date: String,
    #[sea_orm(nullable)]
    pub payment_method: Option<String>,
    #[sea_orm(nullable)]
    pub vendor: Option<String>,
    #[sea_orm(nullable)]
    pub tags: Option<String>,
    #[sea_orm(nullable)]
    pub split_with: Option<String>,
    #[sea_orm(nullable)]
    pub receipt_path: Option<String>,
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
