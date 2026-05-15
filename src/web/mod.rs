use std::sync::Arc;

use axum::{Router, extract::State, response::Html, routing::get};
use sqlx::SqlitePool;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

use crate::{config::Config, error::AppError};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: SqlitePool,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn index(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let meeting_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM meetings")
        .fetch_one(&state.db)
        .await?;

    Ok(Html(format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Google Meeting Recorder</title>
</head>
<body>
  <main>
    <h1>Google Meeting Recorder</h1>
    <p>hello - database is connected, migrations are applied, and {meeting_count} meetings are stored.</p>
    <p>Data directory: {}</p>
  </main>
</body>
</html>"#,
        state.config.storage.data_dir.display()
    )))
}

async fn healthz() -> &'static str {
    "ok"
}
