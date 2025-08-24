use anyhow::Result;
use futures_util::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::Collection;

use crate::db::Db;
use crate::model::Category;

fn coll(db: &Db) -> Collection<Category> {
    db.collection::<Category>("categories")
}

pub async fn list_by_supplier(db: &Db, supplier_id: ObjectId) -> Result<Vec<Category>> {
    let mut cur = coll(db)
        .find(doc! { "supplier_id": supplier_id })
        .await?;
    let mut out = Vec::new();
    while let Some(c) = cur.try_next().await? {
        out.push(c);
    }
    out.sort_by_key(|c| (c.position, c.name.clone()));
    Ok(out)
}

pub async fn create(db: &Db, supplier_id: ObjectId, name: &str) -> Result<ObjectId> {
    let list = list_by_supplier(db, supplier_id).await?;
    let pos = list.last().map(|c| c.position + 1).unwrap_or(0);
    let c = Category {
        id: None,
        supplier_id,
        name: name.to_string(),
        position: pos,
    };
    let r = coll(db).insert_one(c).await?;
    Ok(r.inserted_id.as_object_id().unwrap())
}

pub async fn delete(db: &Db, id: ObjectId) -> Result<()> {
    coll(db).delete_one(doc! { "_id": id }).await?;
    Ok(())
}

pub async fn update(db: &Db, id: ObjectId, name: &str, position: i64) -> Result<()> {
    coll(db)
        .update_one(
            doc! { "_id": id },
            doc! { "$set": { "name": name, "position": position } },
        )
        .await?;
    Ok(())
}
