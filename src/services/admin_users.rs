// src/services/admin_users.rs
use anyhow::{anyhow, Result};
use mongodb::{bson::doc, Collection};
use crate::{db::Db, model::AdminUser};
use crate::auth;

pub async fn count(db: &Db) -> Result<i64> {
    let coll: Collection<AdminUser> = db.db.collection("admin_users");
    Ok(coll.count_documents(doc! {}).await? as i64)
}

pub async fn create(db: &Db, username: &str, plain: &str) -> Result<()> {
    let coll: Collection<AdminUser> = db.db.collection("admin_users");
    let exists = coll.find_one(doc!{"username": username}).await?;
    if exists.is_some() { return Err(anyhow!("user exists")); }
    let hash = auth::hash_password(plain)?;
    let doc = AdminUser { id: None, username: username.into(), password_hash: hash, is_active: true };
    coll.insert_one(doc).await?;
    Ok(())
}

pub async fn verify(db: &Db, username: &str, plain: &str) -> Result<bool> {
    let coll: Collection<AdminUser> = db.db.collection("admin_users");
    if let Some(u) = coll.find_one(doc!{"username": username, "is_active": true}).await? {
        return Ok(auth::verify_password(&u.password_hash, plain)?);
    }
    Ok(false)
}
