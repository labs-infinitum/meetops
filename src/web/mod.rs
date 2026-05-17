use std::sync::Arc;

use axum::{
    Router,
    extract::{Form, Query, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use chrono::{DateTime, Local, Utc};
use reqwest::Client;
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

use crate::{
    config::Config,
    db::{bot_runs::BotRunListItem, models::MeetingListItem},
    error::AppError,
    google,
};

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
        .route("/meetings/auto_join", post(toggle_auto_join))
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

    let bot_runs = crate::db::bot_runs::list_recent(&state.db, 25).await?;

    Ok(Html(render_index(
        connected,
        state.config.bot.join_lead_time_secs,
        state.config.bot.scheduler_poll_interval_secs,
        &meetings,
        &bot_runs,
    )))
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

#[derive(Debug, Deserialize)]
struct ToggleAutoJoinForm {
    meeting_id: String,
    enabled: u8,
}

async fn toggle_auto_join(
    State(state): State<AppState>,
    Form(form): Form<ToggleAutoJoinForm>,
) -> Result<Redirect, AppError> {
    let auto_join = form.enabled != 0;
    let rows =
        crate::db::bot_runs::set_meeting_auto_join(&state.db, &form.meeting_id, auto_join).await?;

    if rows == 0 {
        tracing::warn!(meeting_id = %form.meeting_id, "auto_join toggle: meeting not found");
    }

    Ok(Redirect::to("/"))
}

fn render_index(
    connected: bool,
    join_lead_secs: u64,
    scheduler_poll_secs: u64,
    meetings: &[MeetingListItem],
    bot_runs: &[BotRunListItem],
) -> String {
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
      <th>Auto-record</th>
    </tr>
  </thead>
  <tbody>
    {table_rows}
  </tbody>
</table>"#
        )
    };

    let scheduler_note = format!(
        "When <strong>Auto-record</strong> is on, the server opens Chrome and records the Meet tab under <code>data/recordings/</code> (each run is a <code>run-…</code> folder with <code>raw.webm</code>), starting up to <strong>{join_lead_secs}s before</strong> the calendar start until the calendar end time (poll every <strong>{scheduler_poll_secs}s</strong>). Only one Chrome session runs at a time.",
    );

    let bot_activity = 
    if bot_runs.is_empty() {
        "<p class=\"muted\">No bot runs yet. Scheduled recordings appear here with status (queued → joining → in_meeting → recording → completed).</p>".to_owned()
    } else {
        let run_rows = bot_runs.iter().map(render_bot_run_row).collect::<Vec<_>>().join("");
        format!(
            r#"<table>
  <thead>
    <tr>
      <th>When</th>
      <th>Meeting</th>
      <th>Status</th>
      <th>Recording</th>
      <th>Meeting start</th>
      <th>Started / ended</th>
    </tr>
  </thead>
  <tbody>
    {run_rows}
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
        <p class="muted">{scheduler_note}</p>
      </div>
      <div>{google_status}</div>
    </header>
    {rows}
    <h2 style="margin-top:2.5rem;">Bot activity</h2>
    {bot_activity}
  </main>
</body>
</html>"#
    )
}

fn render_meeting_row(meeting: &MeetingListItem) -> String {
    let (sel_on, sel_off) = if meeting.auto_join == 1 {
        (" selected", "")
    } else {
        ("", " selected")
    };

    format!(
        r#"<tr>
  <td><a href="{meet_url}">{title}</a><br><span class="muted">{id}</span></td>
  <td>{start}</td>
  <td>{end}</td>
  <td>
    <form method="post" action="/meetings/auto_join" style="margin:0;display:flex;gap:0.5rem;align-items:center;flex-wrap:wrap;">
      <input type="hidden" name="meeting_id" value="{id_attr}" />
      <select name="enabled" aria-label="Auto-record">
        <option value="1"{sel_on}>On</option>
        <option value="0"{sel_off}>Off</option>
      </select>
      <button type="submit">Apply</button>
    </form>
  </td>
</tr>"#,
        id = escape_html(&meeting.id),
        id_attr = escape_html(&meeting.id),
        title = escape_html(&meeting.title),
        meet_url = escape_html(&meeting.meet_url),
        start = format_unix(meeting.start_at),
        end = format_unix(meeting.end_at),
        sel_on = sel_on,
        sel_off = sel_off,
    )
}

fn render_bot_run_row(run: &BotRunListItem) -> String {
    let err_cell = match &run.error {
        Some(e) if !e.is_empty() => format!("<br><span class=\"muted\">{}</span>", escape_html(e)),
        _ => String::new(),
    };
    let rec_cell = match &run.recording_path {
        Some(p) if !p.is_empty() => format!("<code>{}</code>", escape_html(p)),
        _ => "<span class=\"muted\">—</span>".to_owned(),
    };
    let started = run
        .started_at
        .map(format_unix)
        .unwrap_or_else(|| "—".to_owned());
    let ended = run
        .ended_at
        .map(format_unix)
        .unwrap_or_else(|| "—".to_owned());

    format!(
        r#"<tr>
  <td>{created}<br><span class="muted">Run #{rid}</span></td>
  <td>{title}<br><span class="muted">{mid}</span></td>
  <td><strong>{status}</strong>{err}</td>
  <td>{rec}</td>
  <td>{mstart}</td>
  <td>{started}<br><span class="muted">{ended}</span></td>
</tr>"#,
        created = format_unix(run.created_at),
        rid = run.id,
        title = escape_html(&run.title),
        mid = escape_html(&run.meeting_id),
        status = escape_html(&run.status),
        err = err_cell,
        rec = rec_cell,
        mstart = format_unix(run.meeting_start_at),
        started = started,
        ended = ended,
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
