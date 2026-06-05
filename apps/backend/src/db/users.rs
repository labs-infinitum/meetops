use crate::db::DbPool;

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
    pool: &DbPool,
    id: &str,
    username: &str,
    email: &str,
    name: &str,
    password_hash: &str,
) -> sqlx::Result<UserRow> {
    let now = super::now_unix();
    sqlx::query(
        r#"
        INSERT INTO users (id, username, email, name, password_hash, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
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

pub async fn get_user_by_id(pool: &DbPool, id: &str) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as("SELECT id, username, email, name, password_hash FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn get_user_by_username(
    pool: &DbPool,
    username: &str,
) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as(
        "SELECT id, username, email, name, password_hash FROM users WHERE lower(username) = lower($1)",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn get_user_by_email(pool: &DbPool, email: &str) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as(
        "SELECT id, username, email, name, password_hash FROM users WHERE lower(email) = lower($1)",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn create_session(
    pool: &DbPool,
    token: &str,
    user_id: &str,
    expires_at: i64,
) -> sqlx::Result<SessionRow> {
    let now = super::now_unix();
    sqlx::query(
        r#"
        INSERT INTO sessions (token, user_id, expires_at, created_at, last_used_at)
        VALUES ($1, $2, $3, $4, $5)
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

pub async fn get_session(pool: &DbPool, token: &str) -> sqlx::Result<Option<SessionRow>> {
    sqlx::query_as(
        r#"
        SELECT token, user_id, expires_at
        FROM sessions
        WHERE token = $1 AND expires_at > $2
        "#,
    )
    .bind(token)
    .bind(super::now_unix())
    .fetch_optional(pool)
    .await
}

pub async fn touch_session(pool: &DbPool, token: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE sessions SET last_used_at = $1 WHERE token = $2")
        .bind(super::now_unix())
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_session(pool: &DbPool, token: &str) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM sessions WHERE token = $1")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_expired_sessions(pool: &DbPool) -> sqlx::Result<u64> {
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= $1")
        .bind(super::now_unix())
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
