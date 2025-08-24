// src/services/dishes.rs
use anyhow::Result;
use mongodb::{bson::{doc, oid::ObjectId}, Collection};
use futures_util::TryStreamExt;
use crate::{db::Db, model::Dish};

pub async fn list_by_supplier(db: &Db, supplier_id: ObjectId) -> Result<Vec<Dish>> {
    let coll: Collection<Dish> = db.db.collection("dishes");
    let mut cur = coll.find(doc!{"supplier_id": supplier_id}).await?;
    let mut out = Vec::new();
    while let Some(d) = cur.try_next().await? { out.push(d); }
    Ok(out)
}

pub async fn create(db: &Db, supplier_id: ObjectId, name: &str, price_cents: i64) -> Result<()> {
    let coll: Collection<Dish> = db.db.collection("dishes");
    let doc = Dish { id: None, supplier_id, name: name.into(), price_cents, is_available: true };
    coll.insert_one(doc).await?;
    Ok(())
}

pub async fn delete(db: &Db, id: ObjectId) -> Result<()> {
    let coll: Collection<Dish> = db.db.collection("dishes");
    coll.delete_one(doc!{"_id": id}).await?;
    Ok(())
}
