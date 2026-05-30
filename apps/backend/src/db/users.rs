use sqlx::SqlitePool;

use crate::db;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    pub email: String,
    pub name: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRow {
    pub token: String,
    pub user_id: String,
    pub expires_at: i64,
}

pub async fn create_user(
    pool: &SqlitePool,
    id: &str,
    username: &str,
    email: &str,
    name: &str,
    password_hash: &str,
) -> sqlx::Result<UserRow> {
    let now = db::now_unix();
    sqlx::query(
        r#"
        INSERT INTO users (id, username, email, name, password_hash, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id)
    .bind(username)
    .bind(email)
    .bind(name)
    .bind(password_hash)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    get_user_by_id(pool, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn get_user_by_id(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as(
        "SELECT id, username, email, name, password_hash FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_user_by_username(pool: &SqlitePool, username: &str) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as(
        "SELECT id, username, email, name, password_hash FROM users WHERE username = ? COLLATE NOCASE",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn get_user_by_email(pool: &SqlitePool, email: &str) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as(
        "SELECT id, username, email, name, password_hash FROM users WHERE email = ? COLLATE NOCASE",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn create_session(
    pool: &SqlitePool,
    token: &str,
    user_id: &str,
    expires_at: i64,
) -> sqlx::Result<SessionRow> {
    let now = db::now_unix();
    sqlx::query(
        r#"
        INSERT INTO sessions (token, user_id, expires_at, created_at, last_used_at)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(token)
    .bind(user_id)
    .bind(expires_at)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    get_session(pool, token)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn get_session(pool: &SqlitePool, token: &str) -> sqlx::Result<Option<SessionRow>> {
    sqlx::query_as(
        r#"
        SELECT token, user_id, expires_at
        FROM sessions
        WHERE token = ? AND expires_at > ?
        "#,
    )
    .bind(token)
    .bind(db::now_unix())
    .fetch_optional(pool)
    .await
}

pub async fn touch_session(pool: &SqlitePool, token: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE sessions SET last_used_at = ? WHERE token = ?")
        .bind(db::now_unix())
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_session(pool: &SqlitePool, token: &str) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM sessions WHERE token = ?")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_expired_sessions(pool: &SqlitePool) -> sqlx::Result<u64> {
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
        .bind(db::now_unix())
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
