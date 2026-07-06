use crate::db::DbPool;

pub const PURPOSE_VERIFY_EMAIL: &str = "verify_email";
pub const PURPOSE_RESET_PASSWORD: &str = "reset_password";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthEmailTokenRow {
    pub token: String,
    pub user_id: String,
}

pub async fn delete_tokens_for_user_purpose(
    pool: &DbPool,
    user_id: &str,
    purpose: &str,
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM auth_email_tokens WHERE user_id = $1 AND purpose = $2")
        .bind(user_id)
        .bind(purpose)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn create_token(
    pool: &DbPool,
    token: &str,
    user_id: &str,
    purpose: &str,
    expires_at: i64,
) -> sqlx::Result<()> {
    let now = super::now_unix();
    sqlx::query(
        r#"
        INSERT INTO auth_email_tokens (token, user_id, purpose, expires_at, created_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(token)
    .bind(user_id)
    .bind(purpose)
    .bind(expires_at)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_valid_token(
    pool: &DbPool,
    token: &str,
    purpose: &str,
) -> sqlx::Result<Option<AuthEmailTokenRow>> {
    sqlx::query_as(
        r#"
        SELECT token, user_id
        FROM auth_email_tokens
        WHERE token = $1 AND purpose = $2 AND expires_at > $3
        "#,
    )
    .bind(token)
    .bind(purpose)
    .bind(super::now_unix())
    .fetch_optional(pool)
    .await
}

pub async fn delete_token(pool: &DbPool, token: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM auth_email_tokens WHERE token = $1")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_expired_tokens(pool: &DbPool) -> sqlx::Result<u64> {
    let result = sqlx::query("DELETE FROM auth_email_tokens WHERE expires_at <= $1")
        .bind(super::now_unix())
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
