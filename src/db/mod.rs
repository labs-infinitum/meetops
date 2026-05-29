use std::str::FromStr;

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::config::Config;

pub mod bot_runs;
pub mod models;

pub use models::BotRunStatus;

pub async fn connect(config: &Config) -> sqlx::Result<SqlitePool> {
    let options =
        SqliteConnectOptions::from_str(&format!("sqlite://{}", config.database_path().display()))?
            .create_if_missing(true)
            .foreign_keys(true);

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
}

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

pub fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

pub async fn get_kv(pool: &SqlitePool, key: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT value FROM app_kv WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
}

pub async fn set_kv(pool: &SqlitePool, key: &str, value: &str) -> sqlx::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO app_kv (key, value, updated_at)
        VALUES (?, ?, ?)
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

pub async fn delete_kv(pool: &SqlitePool, key: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM app_kv WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await?;

    Ok(())
}
