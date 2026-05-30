use std::sync::OnceLock;

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};

const PREFIX: &str = "enc1:";

static KEY: OnceLock<Option<[u8; 32]>> = OnceLock::new();

pub fn init(key: Option<&str>) -> anyhow::Result<()> {
    let parsed = match key.map(str::trim).filter(|value| !value.is_empty()) {
        None => None,
        Some(raw) => {
            let bytes = STANDARD.decode(raw).map_err(|error| {
                anyhow::anyhow!("GMR_STORAGE__TOKEN_ENCRYPTION_KEY must be base64: {error}")
            })?;
            if bytes.len() != 32 {
                anyhow::bail!(
                    "GMR_STORAGE__TOKEN_ENCRYPTION_KEY must decode to 32 bytes (got {})",
                    bytes.len()
                );
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            Some(arr)
        }
    };

    KEY.set(parsed)
        .map_err(|_| anyhow::anyhow!("token encryption already initialized"))?;
    Ok(())
}

pub fn encryption_enabled() -> bool {
    KEY.get().is_some_and(|key| key.is_some())
}

fn key_bytes() -> Option<&'static [u8; 32]> {
    KEY.get().and_then(|key| key.as_ref())
}

pub fn encrypt(plaintext: &str) -> anyhow::Result<String> {
    let Some(key) = key_bytes() else {
        return Ok(plaintext.to_owned());
    };

    let cipher = Aes256Gcm::new(key.into());
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|error| anyhow::anyhow!("token encryption failed: {error}"))?;

    let mut payload = nonce.to_vec();
    payload.extend(ciphertext);
    Ok(format!("{PREFIX}{}", STANDARD.encode(payload)))
}

pub fn decrypt(stored: &str) -> anyhow::Result<String> {
    if !stored.starts_with(PREFIX) {
        return Ok(stored.to_owned());
    }

    let key = key_bytes().ok_or_else(|| {
        anyhow::anyhow!("encrypted calendar token found but GMR_STORAGE__TOKEN_ENCRYPTION_KEY is not set")
    })?;

    let bytes = STANDARD
        .decode(stored.strip_prefix(PREFIX).unwrap_or(stored))
        .map_err(|error| anyhow::anyhow!("invalid encrypted token encoding: {error}"))?;
    if bytes.len() <= 12 {
        anyhow::bail!("invalid encrypted token payload");
    }

    let (nonce_bytes, ciphertext) = bytes.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new(key.into());
    let plain = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|error| anyhow::anyhow!("token decryption failed: {error}"))?;
    Ok(String::from_utf8(plain)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_key() {
        let key = STANDARD.encode([7u8; 32]);
        init(Some(&key)).expect("init key");

        let encrypted = encrypt("refresh-token-value").expect("encrypt");
        assert!(encrypted.starts_with(PREFIX));
        assert_ne!(encrypted, "refresh-token-value");

        let decrypted = decrypt(&encrypted).expect("decrypt");
        assert_eq!(decrypted, "refresh-token-value");
    }
}
