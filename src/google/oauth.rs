use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use reqwest::Client;
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{config::Config, db};

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar.readonly";
const OAUTH_STATE_KEY: &str = "google_oauth_state";

#[derive(Debug, thiserror::Error)]
pub enum GoogleAuthError {
    #[error("Google OAuth is not configured; set google.client_id and google.client_secret")]
    MissingConfig,
    #[error("Google OAuth is not connected")]
    NotConnected,
    #[error("OAuth state did not match; restart the Google connection flow")]
    StateMismatch,
    #[error(
        "Google token response did not include a refresh token; restart auth from /auth/google/start"
    )]
    MissingRefreshToken,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Google token request failed: {0}")]
    Token(String),
}

#[derive(Debug, Clone)]
pub struct AccessToken {
    pub value: String,
}

#[derive(Debug, Deserialize, sqlx::FromRow)]
struct StoredToken {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    scope: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

pub async fn start_url(pool: &SqlitePool, config: &Config) -> Result<String, GoogleAuthError> {
    ensure_config(config)?;

    let state = random_state();
    db::set_kv(pool, OAUTH_STATE_KEY, &state).await?;

    let params = [
        ("client_id", config.google.client_id.as_str()),
        ("redirect_uri", config.google.redirect_uri.as_str()),
        ("response_type", "code"),
        ("scope", CALENDAR_SCOPE),
        ("access_type", "offline"),
        ("prompt", "consent"),
        ("include_granted_scopes", "true"),
        ("state", state.as_str()),
    ];

    let query = params
        .into_iter()
        .map(|(key, value)| format!("{key}={}", urlencoding::encode(value)))
        .collect::<Vec<_>>()
        .join("&");

    Ok(format!("{GOOGLE_AUTH_URL}?{query}"))
}

pub async fn finish_callback(
    pool: &SqlitePool,
    http: &Client,
    config: &Config,
    code: &str,
    state: &str,
) -> Result<(), GoogleAuthError> {
    ensure_config(config)?;
    verify_state(pool, state).await?;

    let response = http
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("code", code),
            ("client_id", config.google.client_id.as_str()),
            ("client_secret", config.google.client_secret.as_str()),
            ("redirect_uri", config.google.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await?;

    let status = response.status();
    let token = response.json::<TokenResponse>().await?;
    if !status.is_success() {
        return Err(GoogleAuthError::Token(token_error(token)));
    }

    let access_token = token
        .access_token
        .ok_or_else(|| GoogleAuthError::Token("missing access_token".to_owned()))?;
    let refresh_token = token
        .refresh_token
        .ok_or(GoogleAuthError::MissingRefreshToken)?;

    let expires_in = token.expires_in.ok_or_else(|| {
        GoogleAuthError::Token("missing expires_in in Google token response".to_owned())
    })?;
    let scope = token
        .scope
        .as_deref()
        .ok_or_else(|| GoogleAuthError::Token("missing scope in Google token response".to_owned()))?;

    store_token(pool, &access_token, &refresh_token, expires_in, scope).await?;

    db::delete_kv(pool, OAUTH_STATE_KEY).await?;

    Ok(())
}

pub async fn valid_access_token(
    pool: &SqlitePool,
    http: &Client,
    config: &Config,
) -> Result<AccessToken, GoogleAuthError> {
    ensure_config(config)?;

    let Some(stored) = load_token(pool).await? else {
        return Err(GoogleAuthError::NotConnected);
    };

    if stored.expires_at > db::now_unix() + 60 {
        return Ok(AccessToken {
            value: stored.access_token,
        });
    }

    let response = http
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("client_id", config.google.client_id.as_str()),
            ("client_secret", config.google.client_secret.as_str()),
            ("refresh_token", stored.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await?;

    let status = response.status();
    let token = response.json::<TokenResponse>().await?;
    if !status.is_success() {
        return Err(GoogleAuthError::Token(token_error(token)));
    }

    let access_token = token
        .access_token
        .ok_or_else(|| GoogleAuthError::Token("missing access_token".to_owned()))?;
    let expires_in = token.expires_in.ok_or_else(|| {
        GoogleAuthError::Token("missing expires_in in Google token response".to_owned())
    })?;
    store_token(
        pool,
        &access_token,
        &stored.refresh_token,
        expires_in,
        token.scope.as_deref().unwrap_or(stored.scope.as_str()),
    )
    .await?;

    Ok(AccessToken {
        value: access_token,
    })
}

pub async fn is_connected(pool: &SqlitePool) -> sqlx::Result<bool> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM google_oauth WHERE id = 1")
        .fetch_one(pool)
        .await?;
    Ok(count > 0)
}

async fn verify_state(pool: &SqlitePool, state: &str) -> Result<(), GoogleAuthError> {
    let expected = db::get_kv(pool, OAUTH_STATE_KEY).await?;
    match expected {
        Some(expected) if expected == state => Ok(()),
        _ => Err(GoogleAuthError::StateMismatch),
    }
}

async fn load_token(pool: &SqlitePool) -> Result<Option<StoredToken>, sqlx::Error> {
    sqlx::query_as::<_, StoredToken>(
        r#"
        SELECT access_token, refresh_token, expires_at, scope
        FROM google_oauth
        WHERE id = 1
        "#,
    )
    .fetch_optional(pool)
    .await
}

async fn store_token(
    pool: &SqlitePool,
    access_token: &str,
    refresh_token: &str,
    expires_in: i64,
    scope: &str,
) -> Result<(), sqlx::Error> {
    let now = db::now_unix();
    let expires_at = now + expires_in;
    sqlx::query(
        r#"
        INSERT INTO google_oauth (id, access_token, refresh_token, expires_at, scope, updated_at)
        VALUES (1, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            access_token = excluded.access_token,
            refresh_token = excluded.refresh_token,
            expires_at = excluded.expires_at,
            scope = excluded.scope,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(access_token)
    .bind(refresh_token)
    .bind(expires_at)
    .bind(scope)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

fn ensure_config(config: &Config) -> Result<(), GoogleAuthError> {
    if config.google.client_id.is_empty() || config.google.client_secret.is_empty() {
        Err(GoogleAuthError::MissingConfig)
    } else {
        Ok(())
    }
}

fn random_state() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn token_error(token: TokenResponse) -> String {
    match (token.error, token.error_description) {
        (Some(error), Some(description)) => format!("{error}: {description}"),
        (Some(error), None) => error,
        _ => "unknown token error".to_owned(),
    }
}
