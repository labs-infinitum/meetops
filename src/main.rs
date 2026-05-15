use std::{net::SocketAddr, sync::Arc};

use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, fmt};

mod config;
mod db;
mod error;
mod web;

use config::Config;
use web::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = Config::load()?;
    tokio::fs::create_dir_all(&config.storage.data_dir).await?;

    let pool = db::connect(&config).await?;
    db::migrate(&pool).await?;

    let addr = SocketAddr::from((config.server.host, config.server.port));
    let state = AppState {
        config: Arc::new(config),
        db: pool,
    };
    let app = web::router(state);
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,google_meeting_recorder=debug,tower_http=info"));

    fmt().with_env_filter(filter).init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
