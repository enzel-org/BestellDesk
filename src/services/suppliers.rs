use anyhow::Result;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::Collection;
use futures_util::TryStreamExt;

use crate::db::Db;
use crate::model::Supplier;

fn coll(db: &Db) -> Collection<Supplier> {
    db.collection::<Supplier>("suppliers")
}

pub async fn list(db: &Db) -> Result<Vec<Supplier>> {
    let c = coll(db);
    let mut cur = c.find(doc! {}).await?;
    let mut out = Vec::new();
    while let Some(s) = cur.try_next().await? {
        out.push(s);
    }
    Ok(out)
}

pub async fn get(db: &Db, id: ObjectId) -> Result<Option<Supplier>> {
    Ok(coll(db).find_one(doc! { "_id": id }).await?)
}

pub async fn create(db: &Db, name: &str, fee_cents: i64) -> Result<ObjectId> {
    let ins = Supplier {
        id: None,
        name: name.to_string(),
        delivery_fee_cents: fee_cents,
    };
    let r = coll(db).insert_one(ins).await?;
    Ok(r.inserted_id.as_object_id().unwrap())
}

pub async fn update(
    db: &Db,
    id: ObjectId,
    name: &str,
    fee_cents: i64,
) -> Result<()> {
    coll(db).update_one(
        doc! { "_id": id },
        doc! { "$set": { "name": name, "delivery_fee_cents": fee_cents } }
    ).await?;
    Ok(())
}

pub async fn delete(db: &Db, id: ObjectId) -> Result<()> {
    coll(db).delete_one(doc! { "_id": id }).await?;
    Ok(())
}
