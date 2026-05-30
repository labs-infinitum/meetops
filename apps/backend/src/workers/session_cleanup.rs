use std::time::Duration;

use sqlx::SqlitePool;

use crate::db::users;

const CLEANUP_INTERVAL_SECS: u64 = 3600;

pub fn spawn(db: SqlitePool) {
    tokio::spawn(async move {
        cleanup(&db).await;

        loop {
            tokio::time::sleep(Duration::from_secs(CLEANUP_INTERVAL_SECS)).await;
            cleanup(&db).await;
        }
    });
}

async fn cleanup(db: &SqlitePool) {
    match users::delete_expired_sessions(db).await {
        Ok(0) => {}
        Ok(deleted) => tracing::info!(deleted, "removed expired sessions"),
        Err(error) => tracing::warn!(%error, "session cleanup failed"),
    }
}
