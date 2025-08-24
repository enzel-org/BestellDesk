use serde::{Deserialize, Serialize};
use mongodb::bson::oid::ObjectId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Supplier {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub delivery_fee_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub active_supplier_id: Option<ObjectId>,
}

/// Admin-Benutzer für das Admin-Panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminUser {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub username: String,
    pub password_hash: String,
}

/// Preisvariante für Pizzen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PizzaSize {
    pub label: String,     // z. B. "26cm", "32cm", "Familie"
    pub price_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dish {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub supplier_id: ObjectId,
    pub name: String,

    /// Generischer Einzelpreis in Cent (für Nicht-Pizza)
    #[serde(default)]
    pub price_cents: i64,

    /// Freie Tags; besondere UI-Logik bei Tag == "Pizza"
    #[serde(default)]
    pub tags: Vec<String>,

    /// Nur bei Pizza: Menü-Nummer (z. B. "P12" oder "12")
    #[serde(default)]
    pub number: Option<String>,

    /// Nur bei Pizza: Varianten mit Größe → Preis
    #[serde(default)]
    pub pizza_sizes: Option<Vec<PizzaSize>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DishInput {
    pub supplier_id: ObjectId,
    pub name: String,
    pub price_cents: Option<i64>,
    pub tags: Vec<String>,
    pub number: Option<String>,
    pub pizza_sizes: Option<Vec<PizzaSize>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub dish_id: ObjectId,
    pub name: String,               // ggf. inkl. Größenlabel
    pub qty: i32,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,

    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub variant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub customer_name: String,
    pub client_id: String,
    pub order_code: String,

    pub supplier_id: ObjectId,
    pub items: Vec<OrderItem>,

    pub delivery_fee_cents: i64,
    pub items_total_cents: i64,
    pub grand_total_cents: i64,

    pub status: String,
    pub created_at: mongodb::bson::DateTime,
}
