use sqlx::SqlitePool;

use crate::{crypto::tokens, db};

fn token_protocol_error(error: anyhow::Error) -> sqlx::Error {
    sqlx::Error::Protocol(error.to_string())
}

fn encrypt_token(value: &str) -> sqlx::Result<String> {
    tokens::encrypt(value).map_err(token_protocol_error)
}

fn decrypt_row(mut row: CalendarConnectionRow) -> sqlx::Result<CalendarConnectionRow> {
    row.access_token = tokens::decrypt(&row.access_token).map_err(token_protocol_error)?;
    row.refresh_token = tokens::decrypt(&row.refresh_token).map_err(token_protocol_error)?;
    Ok(row)
}

#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CalendarConnectionRow {
    pub id: String,
    pub user_id: String,
    pub provider: String,
    pub email: String,
    pub label: String,
    pub color: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub scope: String,
    pub sync_token: Option<String>,
    pub last_synced_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CalendarConnectionListItem {
    pub id: String,
    pub provider: String,
    pub email: String,
    pub label: String,
    pub color: String,
    pub last_synced_at: Option<i64>,
    pub meeting_count: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConnectionOwner {
    pub owner_name: String,
}

pub async fn list_for_user(
    pool: &SqlitePool,
    user_id: &str,
) -> sqlx::Result<Vec<CalendarConnectionListItem>> {
    sqlx::query_as(
        r#"
        SELECT
            cc.id,
            cc.provider,
            cc.email,
            cc.label,
            cc.color,
            cc.last_synced_at,
            COUNT(m.id) AS meeting_count
        FROM calendar_connections cc
        LEFT JOIN meetings m ON m.calendar_connection_id = cc.id
        WHERE cc.user_id = ?
        GROUP BY cc.id
        ORDER BY cc.created_at ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn get_by_id(
    pool: &SqlitePool,
    connection_id: &str,
) -> sqlx::Result<Option<CalendarConnectionRow>> {
    let row = sqlx::query_as("SELECT * FROM calendar_connections WHERE id = ?")
        .bind(connection_id)
        .fetch_optional(pool)
        .await?;

    row.map(decrypt_row).transpose()
}

pub async fn get_by_id_for_user(
    pool: &SqlitePool,
    connection_id: &str,
    user_id: &str,
) -> sqlx::Result<Option<CalendarConnectionRow>> {
    let row = sqlx::query_as("SELECT * FROM calendar_connections WHERE id = ? AND user_id = ?")
        .bind(connection_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

    row.map(decrypt_row).transpose()
}

pub async fn find_by_provider_email(
    pool: &SqlitePool,
    provider: &str,
    email: &str,
) -> sqlx::Result<Option<CalendarConnectionRow>> {
    let row = sqlx::query_as(
        r#"
        SELECT * FROM calendar_connections
        WHERE provider = ? AND email = ? COLLATE NOCASE
        "#,
    )
    .bind(provider)
    .bind(email)
    .fetch_optional(pool)
    .await?;

    row.map(decrypt_row).transpose()
}

pub async fn find_owner_by_provider_email(
    pool: &SqlitePool,
    provider: &str,
    email: &str,
) -> sqlx::Result<Option<ConnectionOwner>> {
    sqlx::query_as(
        r#"
        SELECT u.name AS owner_name
        FROM calendar_connections cc
        JOIN users u ON u.id = cc.user_id
        WHERE cc.provider = ? AND cc.email = ? COLLATE NOCASE
        "#,
    )
    .bind(provider)
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn list_all_for_sync(pool: &SqlitePool) -> sqlx::Result<Vec<CalendarConnectionRow>> {
    let rows = sqlx::query_as(
        r#"
        SELECT * FROM calendar_connections
        WHERE email != ''
        ORDER BY created_at ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(decrypt_row).collect()
}

pub async fn insert(
    pool: &SqlitePool,
    id: &str,
    user_id: &str,
    provider: &str,
    email: &str,
    label: &str,
    color: &str,
    access_token: &str,
    refresh_token: &str,
    expires_at: i64,
    scope: &str,
) -> sqlx::Result<CalendarConnectionRow> {
    let now = db::now_unix();
    let access_token = encrypt_token(access_token)?;
    let refresh_token = encrypt_token(refresh_token)?;
    sqlx::query(
        r#"
        INSERT INTO calendar_connections (
            id, user_id, provider, email, label, color,
            access_token, refresh_token, expires_at, scope,
            created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(provider)
    .bind(email)
    .bind(label)
    .bind(color)
    .bind(access_token)
    .bind(refresh_token)
    .bind(expires_at)
    .bind(scope)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    get_by_id(pool, id).await?.ok_or(sqlx::Error::RowNotFound)
}

pub async fn update_tokens(
    pool: &SqlitePool,
    connection_id: &str,
    access_token: &str,
    refresh_token: &str,
    expires_at: i64,
    scope: &str,
) -> sqlx::Result<()> {
    let now = db::now_unix();
    let access_token = encrypt_token(access_token)?;
    let refresh_token = encrypt_token(refresh_token)?;
    sqlx::query(
        r#"
        UPDATE calendar_connections
        SET access_token = ?, refresh_token = ?, expires_at = ?, scope = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(access_token)
    .bind(refresh_token)
    .bind(expires_at)
    .bind(scope)
    .bind(now)
    .bind(connection_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_sync_state(
    pool: &SqlitePool,
    connection_id: &str,
    sync_token: Option<&str>,
    last_synced_at: i64,
) -> sqlx::Result<()> {
    let now = db::now_unix();
    sqlx::query(
        r#"
        UPDATE calendar_connections
        SET sync_token = ?, last_synced_at = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(sync_token)
    .bind(last_synced_at)
    .bind(now)
    .bind(connection_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_sync_token(pool: &SqlitePool, connection_id: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE calendar_connections SET sync_token = NULL, updated_at = ? WHERE id = ?")
        .bind(db::now_unix())
        .bind(connection_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_for_user(
    pool: &SqlitePool,
    connection_id: &str,
    user_id: &str,
) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM calendar_connections WHERE id = ? AND user_id = ?")
        .bind(connection_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

pub fn next_google_color(existing: &[CalendarConnectionListItem]) -> &'static str {
    const COLORS: &[&str] = &["#4285F4", "#34A853", "#FBBC04", "#EA4335", "#1A73E8"];
    let used: std::collections::HashSet<&str> = existing
        .iter()
        .filter(|c| c.provider == "google")
        .map(|c| c.color.as_str())
        .collect();
    COLORS
        .iter()
        .find(|color| !used.contains(**color))
        .copied()
        .unwrap_or(COLORS[0])
}
