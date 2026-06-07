use aws_sdk_secretsmanager::Client as SecretsClient;
use serde::Deserialize;
use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};

use crate::config::Config;

pub mod bot_runs;
pub mod calendar_connections;
pub mod jobs;
pub mod meeting_timeline;
pub mod meetings;
pub mod models;
pub mod summaries;
pub mod transcripts;
pub mod users;

pub use models::BotRunStatus;

pub type DbPool = PgPool;

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Deserialize)]
struct RdsSecret {
    username: String,
    password: String,
}

pub async fn connect(config: &Config) -> anyhow::Result<DbPool> {
    let url = resolve_database_url(config).await?;
    tracing::info!("connecting to PostgreSQL database");
    Ok(PgPoolOptions::new()
        .max_connections(10)
        .connect(&url)
        .await?)
}

async fn resolve_database_url(config: &Config) -> anyhow::Result<String> {
    if let Some(url) = config
        .database
        .url
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if url.starts_with("sqlite:") {
            anyhow::bail!("SQLite is not supported; use a postgres:// or postgresql:// URL");
        }
        if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
            anyhow::bail!("database URL must use the postgres:// or postgresql:// scheme");
        }
        return Ok(url.to_owned());
    }

    if config
        .database
        .secret_arn
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
    {
        return postgres_url_from_secret(config).await;
    }

    anyhow::bail!(
        "database is not configured: set DATABASE_URL or GMR_DATABASE__SECRET_ARN with GMR_DATABASE__HOST and GMR_DATABASE__DBNAME"
    )
}

async fn postgres_url_from_secret(config: &Config) -> anyhow::Result<String> {
    let secret_arn = config
        .database
        .secret_arn
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("database.secret_arn is not configured"))?;
    let host = config
        .database
        .host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("database.host is required when database.secret_arn is set")
        })?;
    let dbname = config
        .database
        .dbname
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("database.dbname is required when database.secret_arn is set")
        })?;
    let port = config.database.port.unwrap_or(5432);

    let shared_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;
    let client = SecretsClient::new(&shared_config);
    let response = client
        .get_secret_value()
        .secret_id(secret_arn)
        .send()
        .await
        .map_err(|error| anyhow::anyhow!("failed to load database secret: {error}"))?;
    let secret_string = response
        .secret_string()
        .ok_or_else(|| anyhow::anyhow!("database secret {secret_arn} has no string payload"))?;
    let secret: RdsSecret = serde_json::from_str(secret_string)
        .map_err(|error| anyhow::anyhow!("invalid database secret JSON: {error}"))?;
    Ok(format!(
        "postgres://{}:{}@{}:{}/{}",
        urlencoding::encode(&secret.username),
        urlencoding::encode(&secret.password),
        host,
        port,
        dbname
    ))
}

pub async fn migrate(pool: DbPool) -> Result<DbPool, sqlx::migrate::MigrateError> {
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

pub fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

/// SQL fragment: meeting row owned by or linked to `user_bind` (same bind for both).
pub fn sql_meeting_visible_to_user(meeting_alias: &str, user_bind: &str) -> String {
    format!(
        "({meeting_alias}.user_id = {user_bind} OR {meeting_alias}.calendar_connection_id IN (SELECT id FROM calendar_connections WHERE user_id = {user_bind}))"
    )
}

/// SQL fragment: bot run visible to `user_bind` (direct owner or via meeting scope).
pub fn sql_bot_run_visible_to_user(user_bind: &str) -> String {
    format!(
        "(br.user_id = {user_bind} OR {sql_meeting})",
        sql_meeting = sql_meeting_visible_to_user("m", user_bind)
    )
}

pub async fn get_kv(pool: &DbPool, key: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT value FROM app_kv WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
}

pub async fn set_kv(pool: &DbPool, key: &str, value: &str) -> sqlx::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO app_kv (key, value, updated_at)
        VALUES ($1, $2, $3)
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(key)
    .bind(value)
    .bind(now_unix())
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_kv(pool: &DbPool, key: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM app_kv WHERE key = $1")
        .bind(key)
        .execute(pool)
        .await?;

    Ok(())
}
