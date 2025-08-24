// src/services/settings.rs
use anyhow::Result;
use mongodb::{bson::{doc, oid::ObjectId}, Collection};
use crate::{db::Db, model::AppSettings};

pub async fn get(db: &Db) -> Result<Option<AppSettings>> {
    let coll: Collection<AppSettings> = db.db.collection("settings");
    Ok(coll.find_one(doc! {}).await?)
}

pub async fn set_active_supplier(db: &Db, sid: ObjectId) -> Result<()> {
    let coll: Collection<AppSettings> = db.db.collection("settings");
    coll.update_one(
        doc! {},
        doc! { "$set": { "active_supplier_id": sid } }
    ).upsert(true).await?;
    Ok(())
}

pub async fn get_active_supplier_id(db: &Db) -> Result<Option<ObjectId>> {
    Ok(get(db).await?.and_then(|s| s.active_supplier_id))
}
