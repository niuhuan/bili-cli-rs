use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, EntityTrait};
use crate::local::{create_index, index_exists};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "property")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub k: String,
    pub v: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub(crate) async fn init_indexes(db: &DatabaseConnection) {
    if !index_exists(db, "property", "idx_k").await {
        create_index(db, "property", vec!["k"], "idx_k").await;
    }
}
