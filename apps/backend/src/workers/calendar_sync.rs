use std::{sync::Arc, time::Duration};

use crate::db::DbPool;
use reqwest::Client;

use crate::{config::Config, google};

pub fn spawn(config: Arc<Config>, db: DbPool, http: Client) {
    tokio::spawn(async move {
        run_once(&config, &db, &http).await;

        loop {
            tokio::time::sleep(Duration::from_secs(
                config.google.calendar_sync_interval_secs,
            ))
            .await;
            run_once(&config, &db, &http).await;
        }
    });
}

async fn run_once(config: &Config, db: &DbPool, http: &Client) {
    match google::calendar::sync_upcoming_events(db, http, config).await {
        Ok(stats) => tracing::info!(
            seen = stats.seen,
            upserted = stats.upserted,
            deleted = stats.deleted,
            "calendar sync completed"
        ),
        Err(google::calendar::CalendarError::Auth(
            google::oauth::GoogleAuthError::NotConnected,
        )) => {
            tracing::info!("Google Calendar is not connected; connect a calendar in the app");
        }
        Err(google::calendar::CalendarError::Auth(
            google::oauth::GoogleAuthError::MissingConfig,
        )) => {
            tracing::warn!("Google OAuth is not configured; skipping calendar sync");
        }
        Err(error) => tracing::error!(%error, "calendar sync failed"),
    }
}
