use anyhow::{Context, Result};
use aes_gcm::{Aes256Gcm, KeyInit, aead::{Aead, Key, generic_array::GenericArray}};
use argon2::{Argon2, Algorithm, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures_util::TryStreamExt;
use mongodb::bson::{doc, DateTime, Document};

use crate::db::Db;
use std::collections::BTreeMap;

/* ---------- Datenstrukturen ---------- */

#[derive(serde::Serialize, serde::Deserialize)]
struct Meta {
    created_at: DateTime,
    app: String,
    version: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct BackupData {
    meta: Meta,
    collections: BTreeMap<String, Vec<Document>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct EncBlob {
    version: u32,
    kdf: String,    // "argon2id"
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
    salt_b64: String,
    cipher: String, // "aes-256-gcm"
    nonce_b64: String,
    ct_b64: String,
}

/* ---------- Hilfsfunktionen ---------- */

async fn dump_collection(db: &Db, name: &str) -> Result<Vec<Document>> {
    let coll = db.db.collection::<Document>(name);
    let mut cur = coll.find(doc! {}).await?;
    let mut out = Vec::new();
    while let Some(d) = cur.try_next().await? {
        out.push(d);
    }
    Ok(out)
}

fn encrypt(password: &str, plaintext: &[u8]) -> Result<EncBlob> {
    // Argon2id Key-Derivation
    let m_cost = 19_456; // KiB
    let t_cost = 2;
    let p_cost = 1;

    let params = Params::new(m_cost, t_cost, p_cost, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 params: {e}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| anyhow::anyhow!("getrandom salt: {e}"))?;

    let mut key_bytes = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), &salt, &mut key_bytes)
        .map_err(|e| anyhow::anyhow!("argon2 derive: {e}"))?;

    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| anyhow::anyhow!("getrandom nonce: {e}"))?;

    let ct = cipher
        .encrypt(GenericArray::from_slice(&nonce), plaintext)
        .map_err(|_e| anyhow::anyhow!("aes-gcm encrypt failed"))?;

    Ok(EncBlob {
        version: 1,
        kdf: "argon2id".into(),
        m_cost,
        t_cost,
        p_cost,
        salt_b64: B64.encode(&salt),
        cipher: "aes-256-gcm".into(),
        nonce_b64: base64::engine::general_purpose::STANDARD.encode(&nonce),
        ct_b64: base64::engine::general_purpose::STANDARD.encode(&ct),
    })
}

fn decrypt(password: &str, enc: &EncBlob) -> Result<Vec<u8>> {
    anyhow::ensure!(
        enc.kdf == "argon2id" && enc.cipher == "aes-256-gcm",
        "Unsupported backup format"
    );

    let salt = base64::engine::general_purpose::STANDARD
        .decode(&enc.salt_b64)
        .map_err(|e| anyhow::anyhow!("salt b64: {e}"))?;
    let nonce = base64::engine::general_purpose::STANDARD
        .decode(&enc.nonce_b64)
        .map_err(|e| anyhow::anyhow!("nonce b64: {e}"))?;
    let ct = base64::engine::general_purpose::STANDARD
        .decode(&enc.ct_b64)
        .map_err(|e| anyhow::anyhow!("ct b64: {e}"))?;

    let params = Params::new(enc.m_cost, enc.t_cost, enc.p_cost, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 params: {e}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key_bytes = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), &salt, &mut key_bytes)
        .map_err(|e| anyhow::anyhow!("argon2 derive: {e}"))?;

    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let pt = cipher
        .decrypt(GenericArray::from_slice(&nonce), ct.as_ref())
        .map_err(|_e| anyhow::anyhow!("aes-gcm decrypt failed"))?;

    Ok(pt)
}

/* ---------- Public API ---------- */

pub async fn export_to_file(db: &Db, path: &str, password: &str) -> Result<()> {
    // Relevante Collections
    let names = [
        "settings",
        "suppliers",
        "categories",
        "dishes",
        "orders",
        "admin_users",
    ];

    let mut map = BTreeMap::<String, Vec<Document>>::new();
    for n in names {
        map.insert(n.to_string(), dump_collection(db, n).await?);
    }

    let data = BackupData {
        meta: Meta {
            created_at: DateTime::now(),
            app: "BestellDesk".into(),
            version: 1,
        },
        collections: map,
    };

    let json = serde_json::to_vec(&data).context("serialize backup json")?;
    let enc = encrypt(password, &json)?;
    let blob = serde_json::to_vec_pretty(&enc).context("serialize enc blob")?;

    // Sync I/O reicht hier; vermeidet zusätzliche Tokio-Features
    std::fs::write(path, blob).context("write file")?;
    Ok(())
}

pub async fn import_from_file(db: &Db, path: &str, password: &str) -> Result<()> {
    let bytes = std::fs::read(path).context("read file")?;
    let enc: EncBlob = serde_json::from_slice(&bytes).context("parse enc blob")?;
    let pt = decrypt(password, &enc).context("decrypt")?;
    let data: BackupData = serde_json::from_slice(&pt).context("parse backup json")?;

    // Replace all: drop + insert_many
    for (name, docs) in data.collections {
        let _ = db.db.run_command(doc! { "drop": &name }).await; // ignorieren, wenn es die Collection (noch) nicht gibt
        if !docs.is_empty() {
            let coll = db.db.collection::<Document>(&name);
            coll.insert_many(docs)
                .await
                .with_context(|| format!("insert_many into {}", name))?;
        }
    }
    Ok(())
}
