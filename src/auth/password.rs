use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

use super::AuthError;

pub fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| AuthError::Internal(anyhow::Error::msg(error.to_string())))
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, AuthError> {
    let parsed = PasswordHash::new(password_hash)
        .map_err(|error| AuthError::Internal(anyhow::Error::msg(error.to_string())))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::{hash_password, verify_password};

    #[test]
    fn hashes_and_verifies_password() {
        let hash = hash_password("secret-password").expect("hash");
        assert!(verify_password("secret-password", &hash).expect("verify"));
        assert!(!verify_password("wrong", &hash).expect("verify"));
    }
}
