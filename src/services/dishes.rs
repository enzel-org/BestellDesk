use anyhow::Result;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::Collection;
use futures_util::TryStreamExt;

use crate::db::Db;
use crate::model::{Dish, DishInput};

fn coll(db: &Db) -> Collection<Dish> {
    db.collection::<Dish>("dishes")
}

pub async fn list_by_supplier(db: &Db, supplier_id: ObjectId) -> Result<Vec<Dish>> {
    let c = coll(db);
    let mut cur = c.find(doc! { "supplier_id": supplier_id }).await?;
    let mut out = Vec::new();
    while let Some(d) = cur.try_next().await? {
        out.push(d);
    }
    Ok(out)
}

pub async fn get(db: &Db, id: ObjectId) -> Result<Option<Dish>> {
    Ok(coll(db).find_one(doc! { "_id": id }).await?)
}

/// Bestehende Signatur behalten (für generische Gerichte).
pub async fn create(db: &Db, supplier_id: ObjectId, name: &str, price_cents: i64) -> Result<ObjectId> {
    let d = Dish {
        id: None,
        supplier_id,
        name: name.to_string(),
        price_cents,
        tags: vec![],
        number: None,
        pizza_sizes: None,
    };
    let r = coll(db).insert_one(d).await?;
    Ok(r.inserted_id.as_object_id().unwrap())
}

/// Neuer Endpunkt für „Pizza“-Einträge oder beliebige getaggte Gerichte.
pub async fn create_with_tags(db: &Db, input: DishInput) -> Result<ObjectId> {
    let d = Dish {
        id: None,
        supplier_id: input.supplier_id,
        name: input.name,
        price_cents: input.price_cents.unwrap_or(0),
        tags: input.tags,
        number: input.number,
        pizza_sizes: input.pizza_sizes,
    };
    let r = coll(db).insert_one(d).await?;
    Ok(r.inserted_id.as_object_id().unwrap())
}

pub async fn delete(db: &Db, id: ObjectId) -> Result<()> {
    coll(db).delete_one(doc! { "_id": id }).await?;
    Ok(())
}

/// Update-Variante für beide Typen (generic/pizza)
pub async fn update(
    db: &Db,
    id: ObjectId,
    name: &str,
    price_cents: Option<i64>,
    tags: Vec<String>,
    number: Option<String>,
    pizza_sizes: Option<Vec<crate::model::PizzaSize>>,
) -> Result<()> {
    let mut set_doc = doc! { "name": name, "tags": tags };
    if let Some(p) = price_cents {
        set_doc.insert("price_cents", p);
    } else {
        // Kein generischer Preis → 0 setzen
        set_doc.insert("price_cents", 0);
    }
    match number {
        Some(nr) => { set_doc.insert("number", nr); }
        None => { set_doc.insert("number", mongodb::bson::Bson::Null); }
    }
    match pizza_sizes {
        Some(list) => { set_doc.insert("pizza_sizes", mongodb::bson::to_bson(&list)?); }
        None => { set_doc.insert("pizza_sizes", mongodb::bson::Bson::Null); }
    }

    coll(db).update_one(
        doc! { "_id": id },
        doc! { "$set": set_doc }
    ).await?;
    Ok(())
}
