use std::{collections::HashSet, str::FromStr};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::config::Config;

pub mod bot_runs;
pub mod calendar_connections;
pub mod meeting_timeline;
pub mod meetings;
pub mod models;
pub mod users;

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

const MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

const RESET_HINT: &str = "run `cargo run -- db reset` (or set GMR_STORAGE__AUTO_MIGRATE_RESET=true to recreate automatically)";

/// Applies migrations. When `storage.auto_migrate_reset` is enabled, recreates the database file
/// on legacy migration history, checksum mismatches, or a corrupted partial schema.
pub async fn migrate(
    config: &Config,
    pool: SqlitePool,
) -> Result<SqlitePool, sqlx::migrate::MigrateError> {
    if needs_squash_reset(&pool)
        .await
        .map_err(sqlx::migrate::MigrateError::Execute)?
    {
        if !config.storage.auto_migrate_reset {
            tracing::error!("legacy or corrupted schema detected; {RESET_HINT}");
            return Err(migration_reset_required());
        }

        tracing::warn!(
            "legacy or corrupted schema detected; recreating database from 0001_init.sql"
        );
        pool.close().await;
        reset(config).await?;
        return connect(config)
            .await
            .map_err(sqlx::migrate::MigrateError::Execute);
    }

    match MIGRATOR.run(&pool).await {
        Ok(()) => Ok(pool),
        Err(error) if should_reset_on_migrate_error(&error) => {
            if !config.storage.auto_migrate_reset {
                tracing::error!(
                    error = %error,
                    "migration history incompatible with squashed schema; {RESET_HINT}"
                );
                return Err(error);
            }

            tracing::warn!(
                error = %error,
                "migration history incompatible with squashed schema; recreating database"
            );
            pool.close().await;
            reset(config).await?;
            connect(config)
                .await
                .map_err(sqlx::migrate::MigrateError::Execute)
        }
        Err(error) => Err(error),
    }
}

fn migration_reset_required() -> sqlx::migrate::MigrateError {
    sqlx::migrate::MigrateError::Execute(
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("database schema incompatible with current migrations; {RESET_HINT}"),
        )
        .into(),
    )
}

/// Wipes the SQLite file and applies migrations from scratch.
pub async fn reset(config: &Config) -> Result<(), sqlx::migrate::MigrateError> {
    let path = config.database_path();
    if path.exists() {
        tokio::fs::remove_file(&path)
            .await
            .map_err(|error| sqlx::migrate::MigrateError::Execute(error.into()))?;
    }

    let pool = connect(config)
        .await
        .map_err(sqlx::migrate::MigrateError::Execute)?;
    MIGRATOR.run(&pool).await
}

async fn needs_squash_reset(pool: &SqlitePool) -> sqlx::Result<bool> {
    let has_users = table_exists(pool, "users").await?;
    let has_orphan_app_schema = !has_users
        && (table_exists(pool, "meetings").await?
            || table_exists(pool, "calendar_connections").await?
            || table_exists(pool, "bot_runs").await?);
    if has_orphan_app_schema {
        return Ok(true);
    }

    if !has_users {
        return Ok(false);
    }

    if !table_exists(pool, "_sqlx_migrations").await? {
        return Ok(true);
    }

    let applied_versions: Vec<i64> = sqlx::query_scalar("SELECT version FROM _sqlx_migrations")
        .fetch_all(pool)
        .await?;
    if applied_versions.is_empty() {
        return Ok(true);
    }

    let current_versions = MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .collect::<HashSet<_>>();
    let has_unknown_version = applied_versions
        .iter()
        .any(|version| !current_versions.contains(version));

    Ok(has_unknown_version)
}

fn should_reset_on_migrate_error(error: &sqlx::migrate::MigrateError) -> bool {
    matches!(
        error,
        sqlx::migrate::MigrateError::VersionMissing(_)
            | sqlx::migrate::MigrateError::VersionMismatch(_)
            | sqlx::migrate::MigrateError::VersionTooOld(_, _)
    )
}

async fn table_exists(pool: &SqlitePool, name: &str) -> sqlx::Result<bool> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1")
            .bind(name)
            .fetch_optional(pool)
            .await?;

    Ok(exists.is_some())
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

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    use crate::config::Config;

    use super::*;

    fn test_config(data_dir: &PathBuf) -> Config {
        let mut config = Config::default();
        config.storage.data_dir = data_dir.clone();
        config.storage.auto_migrate_reset = true;
        config
    }

    async fn test_pool(config: &Config) -> SqlitePool {
        let options = SqliteConnectOptions::from_str(&format!(
            "sqlite://{}",
            config.database_path().display()
        ))
        .expect("sqlite path")
        .create_if_missing(true)
        .foreign_keys(true);

        SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .expect("connect test database")
    }

    fn test_data_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gmr-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    async fn seed_legacy_migration_chain(pool: &SqlitePool) {
        sqlx::query("CREATE TABLE users (id TEXT PRIMARY KEY)")
            .execute(pool)
            .await
            .expect("seed users table");

        sqlx::query(
            r#"
            CREATE TABLE _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("seed migration table");

        for version in 1..=9 {
            sqlx::query(
                "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
                 VALUES (?, ?, 1, X'00', 0)",
            )
            .bind(version)
            .bind(format!("migration_{version}"))
            .execute(pool)
            .await
            .expect("seed migration row");
        }
    }

    #[tokio::test]
    async fn migrate_resets_legacy_migration_chain() {
        let data_dir = test_data_dir("legacy-chain");
        let _ = std::fs::remove_dir_all(&data_dir);
        std::fs::create_dir_all(&data_dir).expect("create test data dir");
        let config = test_config(&data_dir);
        let _ = std::fs::remove_file(config.database_path());
        let pool = test_pool(&config).await;
        seed_legacy_migration_chain(&pool).await;

        let pool = migrate(&config, pool)
            .await
            .expect("migrate legacy database");

        let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .expect("count migrations");
        assert_eq!(migration_count, 2);

        let version: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .expect("read migration version");
        assert_eq!(version, 2);

        let recording_path_exists: Option<String> = sqlx::query_scalar(
            "SELECT name FROM pragma_table_info('bot_runs') WHERE name = 'recording_path'",
        )
        .fetch_optional(&pool)
        .await
        .expect("inspect bot_runs columns");
        assert_eq!(recording_path_exists.as_deref(), Some("recording_path"));

        pool.close().await;
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn migrate_resets_modified_init_checksum() {
        let data_dir = test_data_dir("checksum-mismatch");
        let _ = std::fs::remove_dir_all(&data_dir);
        std::fs::create_dir_all(&data_dir).expect("create test data dir");
        let config = test_config(&data_dir);
        let _ = std::fs::remove_file(config.database_path());
        let pool = test_pool(&config).await;
        seed_legacy_migration_chain(&pool).await;

        sqlx::query("DELETE FROM _sqlx_migrations WHERE version != 1")
            .execute(&pool)
            .await
            .expect("keep only version 1");

        let pool = migrate(&config, pool)
            .await
            .expect("migrate checksum mismatch database");

        let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .expect("count migrations");
        assert_eq!(migration_count, 2);

        pool.close().await;
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn migrate_accepts_current_migration_chain() {
        let data_dir = test_data_dir("current-chain");
        let _ = std::fs::remove_dir_all(&data_dir);
        std::fs::create_dir_all(&data_dir).expect("create test data dir");
        let mut config = test_config(&data_dir);
        config.storage.auto_migrate_reset = false;
        let _ = std::fs::remove_file(config.database_path());
        let pool = test_pool(&config).await;
        MIGRATOR.run(&pool).await.expect("seed current migrations");

        let pool = migrate(&config, pool)
            .await
            .expect("current migrations should be accepted");

        let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .expect("count migrations");
        assert_eq!(migration_count, MIGRATOR.iter().count() as i64);

        pool.close().await;
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn migrate_refuses_reset_without_flag() {
        let data_dir = test_data_dir("no-auto-reset");
        let _ = std::fs::remove_dir_all(&data_dir);
        std::fs::create_dir_all(&data_dir).expect("create test data dir");
        let mut config = test_config(&data_dir);
        config.storage.auto_migrate_reset = false;
        let _ = std::fs::remove_file(config.database_path());
        let pool = test_pool(&config).await;
        seed_legacy_migration_chain(&pool).await;

        let error = migrate(&config, pool)
            .await
            .expect_err("migrate should fail without auto_migrate_reset");

        assert!(matches!(error, sqlx::migrate::MigrateError::Execute(_)));

        let _ = std::fs::remove_dir_all(data_dir);
    }
}
