use std::sync::Arc;

use axum::{
    Router,
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use chrono::{DateTime, Local, Utc};
use reqwest::Client;
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

use crate::{config::Config, db::models::MeetingListItem, error::AppError, google};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: SqlitePool,
    pub http: Client,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
        .route("/auth/google/start", get(auth_google_start))
        .route("/auth/google/callback", get(auth_google_callback))
        .route("/sync/calendar", post(sync_calendar))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn index(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let connected = google::oauth::is_connected(&state.db).await?;
    let meetings = sqlx::query_as::<_, MeetingListItem>(
        r#"
        SELECT id, title, meet_url, start_at, end_at, auto_join
        FROM meetings
        ORDER BY start_at ASC
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Html(render_index(connected, &meetings)))
}

async fn healthz() -> &'static str {
    "ok"
}

async fn auth_google_start(State(state): State<AppState>) -> Result<Redirect, AppError> {
    let url = google::oauth::start_url(&state.db, &state.config).await?;
    Ok(Redirect::temporary(&url))
}

#[derive(Debug, Deserialize)]
struct GoogleCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn auth_google_callback(
    State(state): State<AppState>,
    Query(query): Query<GoogleCallbackQuery>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(error) = query.error {
        return Ok(Html(render_message("Google authorization failed", &error)));
    }

    let code = query.code.ok_or(google::oauth::GoogleAuthError::Token(
        "missing code".to_owned(),
    ))?;
    let state_value = query
        .state
        .ok_or(google::oauth::GoogleAuthError::StateMismatch)?;

    google::oauth::finish_callback(&state.db, &state.http, &state.config, &code, &state_value)
        .await?;

    let sync_message =
        match google::calendar::sync_upcoming_events(&state.db, &state.http, &state.config).await {
            Ok(stats) => format!(
                "Connected Google Calendar and synced {} Meet events.",
                stats.upserted
            ),
            Err(error) => format!("Connected Google Calendar, but sync failed: {error}"),
        };

    Ok(Html(render_message(
        "Google Calendar connected",
        &sync_message,
    )))
}

async fn sync_calendar(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let stats =
        google::calendar::sync_upcoming_events(&state.db, &state.http, &state.config).await?;
    Ok(Html(render_message(
        "Calendar sync complete",
        &format!(
            "Saw {} events, upserted {} Meet meetings, deleted {} cancelled meetings.",
            stats.seen, stats.upserted, stats.deleted
        ),
    )))
}

fn render_index(connected: bool, meetings: &[MeetingListItem]) -> String {
    let rows = if meetings.is_empty() {
        "<p>No Google Meet events have been synced yet.</p>".to_owned()
    } else {
        let table_rows = meetings
            .iter()
            .map(render_meeting_row)
            .collect::<Vec<_>>()
            .join("");
        format!(
            r#"<table>
  <thead>
    <tr>
      <th>Meeting</th>
      <th>Start</th>
      <th>End</th>
      <th>Auto Join</th>
    </tr>
  </thead>
  <tbody>
    {table_rows}
  </tbody>
</table>"#
        )
    };

    let google_status = if connected {
        r#"<form method="post" action="/sync/calendar">
  <button type="submit">Sync calendar now</button>
</form>"#
            .to_owned()
    } else {
        r#"<p>Google Calendar is not connected.</p>
<p><a href="/auth/google/start">Connect Google Calendar</a></p>"#
            .to_owned()
    };

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Google Meeting Recorder</title>
  <style>
    body {{ font-family: system-ui, sans-serif; margin: 2rem; color: #161616; }}
    main {{ max-width: 960px; margin: 0 auto; }}
    header {{ display: flex; justify-content: space-between; align-items: start; gap: 1rem; }}
    table {{ border-collapse: collapse; width: 100%; margin-top: 1.5rem; }}
    th, td {{ border-bottom: 1px solid #ddd; padding: 0.75rem; text-align: left; vertical-align: top; }}
    th {{ color: #555; font-size: 0.85rem; text-transform: uppercase; }}
    a {{ color: #0645ad; }}
    button {{ cursor: pointer; padding: 0.5rem 0.75rem; }}
    .muted {{ color: #666; }}
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <h1>Google Meeting Recorder</h1>
        <p class="muted">Upcoming Google Meet events synced from your primary calendar.</p>
      </div>
      <div>{google_status}</div>
    </header>
    {rows}
  </main>
</body>
</html>"#
    )
}

fn render_meeting_row(meeting: &MeetingListItem) -> String {
    format!(
        r#"<tr>
  <td><a href="{meet_url}">{title}</a><br><span class="muted">{id}</span></td>
  <td>{start}</td>
  <td>{end}</td>
  <td>{auto_join}</td>
</tr>"#,
        id = escape_html(&meeting.id),
        title = escape_html(&meeting.title),
        meet_url = escape_html(&meeting.meet_url),
        start = format_unix(meeting.start_at),
        end = format_unix(meeting.end_at),
        auto_join = if meeting.auto_join == 1 {
            "enabled"
        } else {
            "disabled"
        }
    )
}

fn render_message(title: &str, message: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
</head>
<body>
  <main>
    <h1>{title}</h1>
    <p>{message}</p>
    <p><a href="/">Back to meetings</a></p>
  </main>
</body>
</html>"#,
        title = escape_html(title),
        message = escape_html(message)
    )
}

fn format_unix(timestamp: i64) -> String {
    DateTime::<Utc>::from_timestamp(timestamp, 0)
        .map(|date| {
            date.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "invalid time".to_owned())
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
