use anyhow::Result;
use mongodb::{options::ClientOptions, Client, Database};
use mongodb::bson::doc;
use mongodb::Collection;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone)]
pub struct Db {
    pub client: Client,
    pub db: Database,
}

pub async fn connect(uri: &str) -> Result<Db> {
    let mut opts = ClientOptions::parse(uri).await?;
    if opts.app_name.is_none() {
        opts.app_name = Some("BestellDesk".into());
    }
    let client = Client::with_options(opts)?;
    let db = client
        .default_database()
        .unwrap_or_else(|| client.database("bestelldesk"));
    db.run_command(doc! { "ping": 1 }).await?;
    Ok(Db { client, db })
}

impl Db {
    // Constrained so it matches mongodb::Database::collection bounds.
    pub fn collection<T: Send + Sync>(&self, name: &str) -> Collection<T> {
        self.db.collection::<T>(name)
    }
}

// Watcher für Settings
pub async fn watch_settings(db: Db, tx: UnboundedSender<crate::AppMsg>) {
    let coll = db.collection::<crate::model::AppSettings>("settings");
    let mut stream = match coll.watch().await {
        Ok(s) => s,
        Err(_) => return,
    };
    while let Some(_ev) =
        futures_util::TryStreamExt::try_next(&mut stream).await.ok().flatten()
    {
        let _ = tx.send(crate::AppMsg::SettingsChanged);
    }
}

// Watcher für Suppliers
pub async fn watch_suppliers(db: Db, tx: UnboundedSender<crate::AppMsg>) {
    let coll = db.collection::<crate::model::Supplier>("suppliers");
    let mut stream = match coll.watch().await {
        Ok(s) => s,
        Err(_) => return,
    };
    while let Some(_ev) =
        futures_util::TryStreamExt::try_next(&mut stream).await.ok().flatten()
    {
        let _ = tx.send(crate::AppMsg::SuppliersChanged);
    }
}

// Watcher für Dishes
pub async fn watch_dishes(db: Db, tx: UnboundedSender<crate::AppMsg>) {
    let coll = db.collection::<crate::model::Dish>("dishes");
    let mut stream = match coll.watch().await {
        Ok(s) => s,
        Err(_) => return,
    };
    while let Some(_ev) =
        futures_util::TryStreamExt::try_next(&mut stream).await.ok().flatten()
    {
        let _ = tx.send(crate::AppMsg::DishesChanged);
    }
}

// Watcher für Orders
pub async fn watch_orders(db: Db, tx: UnboundedSender<crate::AppMsg>) {
    let coll = db.collection::<crate::model::Order>("orders");
    let mut stream = match coll.watch().await {
        Ok(s) => s,
        Err(_) => return,
    };
    while let Some(_ev) =
        futures_util::TryStreamExt::try_next(&mut stream).await.ok().flatten()
    {
        let _ = tx.send(crate::AppMsg::OrdersChanged);
    }
}
