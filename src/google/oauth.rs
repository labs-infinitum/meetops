use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{config::Config, db::calendar_connections, db};

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
pub const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar.readonly";
pub const USERINFO_EMAIL_SCOPE: &str = "https://www.googleapis.com/auth/userinfo.email";
pub const OAUTH_SCOPES: &str =
    "https://www.googleapis.com/auth/calendar.readonly https://www.googleapis.com/auth/userinfo.email";

fn oauth_state_key(state: &str) -> String {
    format!("oauth_state:{state}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthStatePayload {
    pub user_id: Option<String>,
    pub provider: String,
    pub label: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum GoogleAuthError {
    #[error("Google OAuth is not configured; set google.client_id and google.client_secret")]
    MissingConfig,
    #[error("Google OAuth is not connected")]
    NotConnected,
    #[error("OAuth state did not match; restart the Google connection flow")]
    StateMismatch,
    #[error(
        "Google token response did not include a refresh token; restart calendar connect from the app"
    )]
    MissingRefreshToken,
    #[error("Google account email is unavailable; grant email permission and try again")]
    MissingEmail,
    #[error("calendar account {email} already linked to another user")]
    AccountLinkedToOtherUser { email: String },
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

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    email: Option<String>,
}

pub async fn start_url(
    pool: &SqlitePool,
    config: &Config,
    payload: OAuthStatePayload,
) -> Result<String, GoogleAuthError> {
    ensure_config(config)?;

    let state = random_state();
    let json = serde_json::to_string(&payload).map_err(|error| {
        GoogleAuthError::Token(format!("failed to serialize oauth state: {error}"))
    })?;
    db::set_kv(pool, &oauth_state_key(&state), &json).await?;

    let scope = if payload.user_id.is_some() {
        OAUTH_SCOPES
    } else {
        CALENDAR_SCOPE
    };

    let params = [
        ("client_id", config.google.client_id.as_str()),
        ("redirect_uri", config.google.redirect_uri.as_str()),
        ("response_type", "code"),
        ("scope", scope),
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
) -> Result<calendar_connections::CalendarConnectionRow, GoogleAuthError> {
    ensure_config(config)?;
    let payload = verify_state(pool, state).await?;

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
        .unwrap_or(OAUTH_SCOPES);

    let email = if payload.user_id.is_some() {
        fetch_google_email(http, &access_token).await?
    } else {
        String::new()
    };

    db::delete_kv(pool, &oauth_state_key(state)).await?;

    let expires_at = db::now_unix() + expires_in;

    if let Some(user_id) = payload.user_id.as_deref() {
        if email.is_empty() {
            return Err(GoogleAuthError::MissingEmail);
        }

        if let Some(existing) =
            calendar_connections::find_by_provider_email(pool, &payload.provider, &email).await?
        {
            match existing.user_id.as_deref() {
                Some(existing_user) if existing_user == user_id => {
                    calendar_connections::update_tokens(
                        pool,
                        &existing.id,
                        &access_token,
                        &refresh_token,
                        expires_at,
                        scope,
                    )
                    .await?;
                    return calendar_connections::get_by_id(pool, &existing.id)
                        .await?
                        .ok_or(GoogleAuthError::NotConnected);
                }
                Some(_) => {
                    return Err(GoogleAuthError::AccountLinkedToOtherUser { email });
                }
                None => {
                    // Orphan legacy connection — assign to this user.
                    let now = db::now_unix();
                    sqlx::query(
                        r#"
                        UPDATE calendar_connections
                        SET user_id = ?, email = ?, access_token = ?, refresh_token = ?,
                            expires_at = ?, scope = ?, updated_at = ?
                        WHERE id = ?
                        "#,
                    )
                    .bind(user_id)
                    .bind(&email)
                    .bind(&access_token)
                    .bind(&refresh_token)
                    .bind(expires_at)
                    .bind(scope)
                    .bind(now)
                    .bind(&existing.id)
                    .execute(pool)
                    .await?;
                    return calendar_connections::get_by_id(pool, &existing.id)
                        .await?
                        .ok_or(GoogleAuthError::NotConnected);
                }
            }
        }

        let label = payload
            .label
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| email.split('@').next().unwrap_or("Google").to_owned());

        let existing_for_user = calendar_connections::list_for_user(pool, user_id).await?;
        let color = payload.color.unwrap_or_else(|| {
            calendar_connections::next_google_color(&existing_for_user).to_owned()
        });

        let connection_id = Uuid::new_v4().to_string();
        return calendar_connections::insert(
            pool,
            &connection_id,
            user_id,
            &payload.provider,
            &email,
            &label,
            &color,
            &access_token,
            &refresh_token,
            expires_at,
            scope,
        )
        .await
        .map_err(GoogleAuthError::from);
    }

    // Legacy single-tenant callback without a logged-in user.
    if let Some(existing) = calendar_connections::list_all_for_sync(pool).await?.into_iter().next()
    {
        calendar_connections::update_tokens(
            pool,
            &existing.id,
            &access_token,
            &refresh_token,
            expires_at,
            scope,
        )
        .await?;
        return calendar_connections::get_by_id(pool, &existing.id)
            .await?
            .ok_or(GoogleAuthError::NotConnected);
    }

    let connection_id = Uuid::new_v4().to_string();
    insert_legacy_connection(
        pool,
        &connection_id,
        &access_token,
        &refresh_token,
        expires_at,
        scope,
    )
    .await
}

async fn insert_legacy_connection(
    pool: &SqlitePool,
    connection_id: &str,
    access_token: &str,
    refresh_token: &str,
    expires_at: i64,
    scope: &str,
) -> Result<calendar_connections::CalendarConnectionRow, GoogleAuthError> {
    let now = db::now_unix();
    sqlx::query(
        r#"
        INSERT INTO calendar_connections (
            id, user_id, provider, email, label, color,
            access_token, refresh_token, expires_at, scope,
            created_at, updated_at
        )
        VALUES (?, NULL, 'google', '', 'Google Calendar', '#4285F4', ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(connection_id)
    .bind(access_token)
    .bind(refresh_token)
    .bind(expires_at)
    .bind(scope)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    calendar_connections::get_by_id(pool, connection_id)
        .await?
        .ok_or(GoogleAuthError::NotConnected)
}

async fn fetch_google_email(http: &Client, access_token: &str) -> Result<String, GoogleAuthError> {
    let response = http
        .get(GOOGLE_USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(GoogleAuthError::MissingEmail);
    }

    let info = response.json::<GoogleUserInfo>().await?;
    info.email
        .map(|email| calendar_connections::normalize_email(&email))
        .filter(|email| !email.is_empty())
        .ok_or(GoogleAuthError::MissingEmail)
}

pub async fn valid_access_token_for_connection(
    pool: &SqlitePool,
    http: &Client,
    config: &Config,
    connection_id: &str,
) -> Result<AccessToken, GoogleAuthError> {
    ensure_config(config)?;

    let Some(stored) = calendar_connections::get_by_id(pool, connection_id).await? else {
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

    calendar_connections::update_tokens(
        pool,
        connection_id,
        &access_token,
        &stored.refresh_token,
        db::now_unix() + expires_in,
        token.scope.as_deref().unwrap_or(stored.scope.as_str()),
    )
    .await?;

    Ok(AccessToken {
        value: access_token,
    })
}

async fn verify_state(pool: &SqlitePool, state: &str) -> Result<OAuthStatePayload, GoogleAuthError> {
    let stored = db::get_kv(pool, &oauth_state_key(state)).await?;
    let Some(json) = stored else {
        return Err(GoogleAuthError::StateMismatch);
    };
    serde_json::from_str(&json).map_err(|_| GoogleAuthError::StateMismatch)
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
