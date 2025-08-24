use anyhow::Result;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};

use crate::db::Db;

/// Create an order document in the "orders" collection.
/// items: Vec<(dish_id, dish_name, qty, unit_price_cents)>
pub async fn create(
    db: &Db,
    customer_name: &str,
    supplier_id: ObjectId,
    items: Vec<(ObjectId, String, i32, i64)>,
    delivery_fee_cents: i64,
    client_id: &str,
) -> Result<ObjectId> {
    let code = nanoid::nanoid!(8);

    let items_docs: Vec<Document> = items
        .iter()
        .map(|(dish_id, name, qty, unit)| {
            doc! {
                "dish_id": dish_id,
                "name": name,
                "qty": qty,
                "unit_price_cents": unit,
                "line_total_cents": (unit * (*qty as i64)),
            }
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

    // FIX: collection-Handle definieren
    let coll = db.db.collection::<Document>("orders");
    let r = coll.insert_one(order_doc).await?;
    Ok(r.inserted_id.as_object_id().unwrap())
}
