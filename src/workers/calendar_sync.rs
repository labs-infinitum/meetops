use std::{net::IpAddr, sync::Arc, time::Duration};

use reqwest::Client;
use sqlx::SqlitePool;

use crate::{config::Config, google};

pub fn spawn(config: Arc<Config>, db: SqlitePool, http: Client) {
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

async fn run_once(config: &Config, db: &SqlitePool, http: &Client) {
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
            tracing::info!(
                auth_url = %auth_start_url(config),
                "Google Calendar is not connected; visit auth_url to connect it"
            );
        }
        Err(google::calendar::CalendarError::Auth(
            google::oauth::GoogleAuthError::MissingConfig,
        )) => {
            tracing::warn!("Google OAuth is not configured; skipping calendar sync");
        }
        Err(error) => tracing::error!(%error, "calendar sync failed"),
    }
}

fn auth_start_url(config: &Config) -> String {
    let host = match config.server.host {
        IpAddr::V4(addr) if addr.is_unspecified() => "127.0.0.1".to_owned(),
        IpAddr::V6(addr) if addr.is_unspecified() => "[::1]".to_owned(),
        IpAddr::V6(addr) => format!("[{addr}]"),
        IpAddr::V4(addr) => addr.to_string(),
    };

    format!("http://{}:{}/auth/google/start", host, config.server.port)
}
