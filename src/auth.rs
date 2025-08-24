use anyhow::anyhow;

/// Password hashing and verification helpers for admin users.
pub fn hash_password(plain: &str) -> anyhow::Result<String> {
    use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
    use rand::rngs::OsRng;

    // Generate a cryptographically secure random salt.
    let salt = SaltString::generate(&mut OsRng);

    // Hash the password using Argon2 (default params are fine for a desktop app).
    let hash = Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| anyhow!("{e}"))?
        .to_string();

    Ok(hash)
}

pub fn verify_password(hash: &str, plain: &str) -> anyhow::Result<bool> {
    use argon2::{
        password_hash::{PasswordHash, PasswordVerifier},
        Argon2,
    };

    // Parse stored hash format; map error to anyhow.
    let parsed = PasswordHash::new(hash).map_err(|e| anyhow!("{e}"))?;

    // Return true if verification succeeds.
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}
