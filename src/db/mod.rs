use std::str::FromStr;

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::config::Config;

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
