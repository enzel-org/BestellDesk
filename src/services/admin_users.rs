use anyhow::Result;
use mongodb::bson::doc;
use mongodb::Collection;

use crate::auth;
use crate::db::Db;
use crate::model::AdminUser;

fn coll(db: &Db) -> Collection<AdminUser> {
    db.collection::<AdminUser>("admin_users")
}

pub async fn count(db: &Db) -> Result<i64> {
    let n = coll(db).count_documents(doc! {}).await? as i64;
    Ok(n)
}

pub async fn create(db: &Db, user: &str, pass: &str) -> Result<()> {
    let hash = auth::hash_password(pass)?;
    let doc = AdminUser {
        id: None,
        username: user.to_string(),
        password_hash: hash,
    };
    coll(db).insert_one(doc).await?;
    Ok(())
}

pub async fn verify(db: &Db, user: &str, pass: &str) -> Result<bool> {
    if let Some(u) = coll(db).find_one(doc! { "username": user }).await? {
        Ok(auth::verify_password(&u.password_hash, pass)?)
    } else {
        Ok(false)
    }
}
