use anyhow::Result;
use futures_util::TryStreamExt;
use mongodb::{
    bson::{doc, oid::ObjectId},
    Collection,
};

use crate::{db::Db, model::Supplier};

pub async fn list(db: &Db) -> Result<Vec<Supplier>> {
    let coll: Collection<Supplier> = db.db.collection("suppliers");
    let mut cur = coll.find(doc! {}).await?;
    let mut out = Vec::new();
    while let Some(s) = cur.try_next().await? {
        out.push(s);
    }
    Ok(out)
}

pub async fn get_supplier(db: &Db, id: ObjectId) -> Result<Option<Supplier>> {
    let coll: Collection<Supplier> = db.db.collection("suppliers");
    Ok(coll.find_one(doc! { "_id": id }).await?)
}

pub async fn create(db: &Db, name: &str, fee_cents: i64) -> Result<ObjectId> {
    let coll: Collection<Supplier> = db.db.collection("suppliers");
    let ins = Supplier {
        id: None,
        name: name.into(),
        delivery_fee_cents: fee_cents,
        is_active: false, // DB braucht Feld, aber UI zeigt es nicht
    };
    let r = coll.insert_one(ins).await?;
    Ok(r.inserted_id.as_object_id().unwrap())
}

pub async fn update(
    db: &Db,
    id: ObjectId,
    name: &str,
    fee_cents: i64,
) -> Result<()> {
    let coll: Collection<Supplier> = db.db.collection("suppliers");
    coll.update_one(
        doc! { "_id": id },
        doc! { "$set": {
            "name": name,
            "delivery_fee_cents": fee_cents
        }},
    )
    .await?;
    Ok(())
}

pub async fn delete(db: &Db, id: ObjectId) -> Result<()> {
    let coll: Collection<Supplier> = db.db.collection("suppliers");
    coll.delete_one(doc! { "_id": id }).await?;
    Ok(())
}
