use anyhow::Result;
use futures_util::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::Collection;

use crate::db::Db;
use crate::model::Order;

fn coll(db: &Db) -> Collection<Order> {
    db.collection::<Order>("orders")
}

pub async fn list_by_supplier(db: &Db, supplier_id: ObjectId) -> Result<Vec<Order>> {
    let mut cur = coll(db)
        .find(doc! { "supplier_id": supplier_id })
        .await?;
    let mut out = Vec::new();
    while let Some(o) = cur.try_next().await? {
        out.push(o);
    }
    // created_at DESC (DateTime ist NICHT Option)
    out.sort_by_key(|o| std::cmp::Reverse(o.created_at.timestamp_millis()));
    Ok(out)
}

pub async fn set_paid_cents(db: &Db, id: ObjectId, paid_cents: i64, completed: bool) -> Result<()> {
    coll(db)
        .update_one(
            doc! { "_id": id },
            doc! { "$set": { "paid_cents": paid_cents, "completed": completed } },
        )
        .await?;
    Ok(())
}

pub async fn delete(db: &Db, id: ObjectId) -> Result<()> {
    coll(db).delete_one(doc! { "_id": id }).await?;
    Ok(())
}

pub async fn create_with_notes(
    db: &Db,
    customer_name: &str,
    supplier_id: ObjectId,
    items: Vec<(ObjectId, String, i32, i64, Option<String>, Option<String>)>,
    delivery_fee_cents: i64,
    client_id: &str,
) -> Result<ObjectId> {
    let code = nanoid::nanoid!(8);

    let items_docs: Vec<Document> = items
        .iter()
        .map(|(dish_id, name, qty, unit, note, variant)| {
            let mut d = doc! {
                "dish_id": dish_id,
                "name": name,
                "qty": qty,
                "unit_price_cents": unit,
                "line_total_cents": (unit * (*qty as i64)),
            };
            if let Some(n) = note { if !n.trim().is_empty() { d.insert("note", n); } }
            if let Some(v) = variant { d.insert("variant", v); }
            d
        })
        .collect();

    let items_total_cents: i64 = items_docs
        .iter()
        .map(|d| d.get_i64("line_total_cents").unwrap_or(0))
        .sum();

    let grand_total_cents = items_total_cents + delivery_fee_cents;

    let order_doc = doc! {
        "customer_name": customer_name,
        "client_id": client_id,
        "order_code": code,
        "supplier_id": supplier_id,
        "items": items_docs,
        "delivery_fee_cents": delivery_fee_cents,
        "items_total_cents": items_total_cents,
        "grand_total_cents": grand_total_cents,
        "created_at": DateTime::now(),
        "status": "new",
    };

    let r = db.collection::<mongodb::bson::Document>("orders").insert_one(order_doc).await?;
    Ok(r.inserted_id.as_object_id().unwrap())
}
