use anyhow::Result;
use futures_util::TryStreamExt;
use mongodb::bson::{self, doc, oid::ObjectId};
use mongodb::Collection;

use crate::db::Db;
use crate::model::{Dish, DishInput, PizzaSize};

fn coll(db: &Db) -> Collection<Dish> {
    db.collection::<Dish>("dishes")
}

pub async fn list_by_supplier(db: &Db, supplier_id: ObjectId) -> Result<Vec<Dish>> {
    let mut cur = coll(db).find(doc! { "supplier_id": supplier_id }).await?;
    let mut out = Vec::new();
    while let Some(d) = cur.try_next().await? {
        out.push(d);
    }
    Ok(out)
}

pub async fn get(db: &Db, id: ObjectId) -> Result<Option<Dish>> {
    Ok(coll(db).find_one(doc! { "_id": id }).await?)
}

pub async fn create(
    db: &Db,
    supplier_id: ObjectId,
    name: &str,
    price_cents: i64,
) -> Result<ObjectId> {
    let d = Dish {
        id: None,
        supplier_id,
        name: name.to_string(),
        price_cents,
        tags: vec![],
        number: None,
        pizza_sizes: None,
        categories: Vec::new(),
    };
    let r = coll(db).insert_one(d).await?;
    Ok(r.inserted_id.as_object_id().unwrap())
}

pub async fn create_plain(
    db: &Db,
    supplier_id: ObjectId,
    name: &str,
    number: Option<String>,
    price_cents: i64,
    categories: Vec<ObjectId>,
) -> Result<ObjectId> {
    let d = Dish {
        id: None,
        supplier_id,
        name: name.to_string(),
        price_cents,
        tags: vec![],
        number,
        pizza_sizes: None,
        categories,
    };
    let r = coll(db).insert_one(d).await?;
    Ok(r.inserted_id.as_object_id().unwrap())
}

pub async fn create_with_tags(db: &Db, input: DishInput) -> Result<ObjectId> {
    let d = Dish {
        id: None,
        supplier_id: input.supplier_id,
        name: input.name,
        price_cents: input.price_cents.unwrap_or(0),
        tags: input.tags,
        number: input.number,
        pizza_sizes: input.pizza_sizes,
        categories: input.categories.unwrap_or_default(),
    };
    let r = coll(db).insert_one(d).await?;
    Ok(r.inserted_id.as_object_id().unwrap())
}

pub async fn delete(db: &Db, id: ObjectId) -> Result<()> {
    coll(db).delete_one(doc! { "_id": id }).await?;
    Ok(())
}

pub async fn update_plain(
    db: &Db,
    id: ObjectId,
    name: &str,
    number: Option<String>,
    price_cents: i64,
    categories: Vec<ObjectId>,
) -> Result<()> {
    coll(db)
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "name": name,
                "number": number,
                "price_cents": price_cents,
                "pizza_sizes": bson::Bson::Null,
                "tags": [],
                "categories": categories,
            }},
        )
        .await?;
    Ok(())
}

pub async fn update_pizza(
    db: &Db,
    id: ObjectId,
    name: &str,
    number: Option<String>,
    sizes: Vec<PizzaSize>,
    categories: Vec<ObjectId>,
) -> Result<()> {
    coll(db)
        .update_one(
            doc! { "_id": id },
            doc! { "$set": {
                "name": name,
                "number": number,
                "pizza_sizes": bson::to_bson(&sizes)?,
                "price_cents": 0,
                "tags": ["Pizza"],
                "categories": categories,
            }},
        )
        .await?;
    Ok(())
}
