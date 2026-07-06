use std::time::Duration;

use crate::db::DbPool;

use crate::db::{auth_email_tokens, users};

const CLEANUP_INTERVAL_SECS: u64 = 3600;

pub fn spawn(db: DbPool) {
    tokio::spawn(async move {
        cleanup(&db).await;

        loop {
            tokio::time::sleep(Duration::from_secs(CLEANUP_INTERVAL_SECS)).await;
            cleanup(&db).await;
        }
    });
}

async fn cleanup(db: &DbPool) {
    match users::delete_expired_sessions(db).await {
        Ok(0) => {}
        Ok(deleted) => tracing::info!(deleted, "removed expired sessions"),
        Err(error) => tracing::warn!(%error, "session cleanup failed"),
    }
    match auth_email_tokens::delete_expired_tokens(db).await {
        Ok(0) => {}
        Ok(deleted) => tracing::info!(deleted, "removed expired auth email tokens"),
        Err(error) => tracing::warn!(%error, "auth email token cleanup failed"),
    }
}
