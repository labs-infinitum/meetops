use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::{
    config::Config,
    db::{self, calendar_connections::CalendarConnectionRow},
    google::oauth,
    meet_url::MeetUrl,
};

const CALENDAR_EVENTS_URL: &str = "https://www.googleapis.com/calendar/v3/calendars/primary/events";

#[derive(Debug, thiserror::Error)]
pub enum CalendarError {
    #[error(transparent)]
    Auth(#[from] oauth::GoogleAuthError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Calendar API request failed: {0}")]
    Api(String),
    #[error("Calendar event {event_id} has an unsupported datetime: {value}")]
    InvalidDateTime { event_id: String, value: String },
}

#[derive(Debug, Clone, Copy)]
pub struct SyncStats {
    pub seen: usize,
    pub upserted: usize,
    pub deleted: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventsResponse {
    #[serde(default)]
    items: Vec<GoogleEvent>,
    next_page_token: Option<String>,
    next_sync_token: Option<String>,
    error: Option<GoogleApiError>,
}

#[derive(Debug, Deserialize)]
struct GoogleApiError {
    code: Option<i64>,
    message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalendarAttendee {
    email: Option<String>,
    display_name: Option<String>,
    response_status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleEvent {
    id: String,
    status: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    organizer: Option<EventPerson>,
    hangout_link: Option<String>,
    conference_data: Option<ConferenceData>,
    start: EventDateTime,
    end: EventDateTime,
    attendees: Option<Vec<CalendarAttendee>>,
    etag: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EventPerson {
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConferenceData {
    entry_points: Option<Vec<EntryPoint>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryPoint {
    entry_point_type: Option<String>,
    uri: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventDateTime {
    date_time: Option<String>,
    date: Option<String>,
}

pub async fn sync_upcoming_events(
    pool: &SqlitePool,
    http: &Client,
    config: &Config,
) -> Result<SyncStats, CalendarError> {
    let mut total = SyncStats {
        seen: 0,
        upserted: 0,
        deleted: 0,
    };

    let connections = db::calendar_connections::list_all_for_sync(pool).await?;
    if connections.is_empty() {
        return Err(CalendarError::Auth(oauth::GoogleAuthError::NotConnected));
    }

    for connection in connections {
        let stats = sync_connection(pool, http, config, &connection).await?;
        total.seen += stats.seen;
        total.upserted += stats.upserted;
        total.deleted += stats.deleted;
    }

    Ok(total)
}

pub async fn sync_connection(
    pool: &SqlitePool,
    http: &Client,
    config: &Config,
    connection: &CalendarConnectionRow,
) -> Result<SyncStats, CalendarError> {
    let access_token =
        oauth::valid_access_token_for_connection(pool, http, config, &connection.id).await?;
    let sync_token = connection.sync_token.clone();

    match sync_with_token(pool, http, connection, &access_token.value, sync_token.as_deref()).await
    {
        Ok(stats) => Ok(stats),
        Err(CalendarError::Api(message))
            if message.contains("410") && sync_token.is_some() =>
        {
            tracing::warn!(
                connection_id = %connection.id,
                "Google Calendar sync token expired; running a full sync"
            );
            db::calendar_connections::delete_sync_token(pool, &connection.id).await?;
            let access_token =
                oauth::valid_access_token_for_connection(pool, http, config, &connection.id)
                    .await?;
            sync_with_token(pool, http, connection, &access_token.value, None).await
        }
        Err(error) => Err(error),
    }
}

async fn sync_with_token(
    pool: &SqlitePool,
    http: &Client,
    connection: &CalendarConnectionRow,
    access_token: &str,
    sync_token: Option<&str>,
) -> Result<SyncStats, CalendarError> {
    let mut stats = SyncStats {
        seen: 0,
        upserted: 0,
        deleted: 0,
    };
    let mut page_token: Option<String> = None;

    let final_sync_token = loop {
        let mut request = http.get(CALENDAR_EVENTS_URL).bearer_auth(access_token);
        request = request.query(&[("singleEvents", "true"), ("maxResults", "250")]);

        if let Some(sync_token) = sync_token {
            request = request.query(&[("syncToken", sync_token)]);
        } else {
            let now = Utc::now();
            let time_min = now.to_rfc3339();
            let time_max = (now + Duration::days(14)).to_rfc3339();
            request = request.query(&[
                ("timeMin", time_min.as_str()),
                ("timeMax", time_max.as_str()),
                ("orderBy", "startTime"),
            ]);
        }

        if let Some(page_token) = &page_token {
            request = request.query(&[("pageToken", page_token.as_str())]);
        }

        let response = request.send().await?;
        let status = response.status();
        let body = response.json::<EventsResponse>().await?;

        if !status.is_success() {
            let message = body
                .error
                .and_then(|error| match (error.code, error.message) {
                    (Some(code), Some(message)) => Some(format!("{code}: {message}")),
                    (Some(code), None) => Some(code.to_string()),
                    (None, Some(message)) => Some(message),
                    (None, None) => None,
                })
                .unwrap_or_else(|| status.to_string());
            return Err(CalendarError::Api(message));
        }

        stats.seen += body.items.len();
        for event in body.items {
            if event.status.as_deref() == Some("cancelled") {
                stats.deleted += delete_meeting(pool, connection, &event.id).await?;
                continue;
            }

            let Some(meet_url) = meet_url(&event) else {
                continue;
            };

            upsert_meeting(pool, connection, &event, &meet_url).await?;
            stats.upserted += 1;
        }

        if let Some(next_page_token) = body.next_page_token {
            page_token = Some(next_page_token);
        } else {
            break body.next_sync_token;
        }
    };

    let now = db::now_unix();
    db::calendar_connections::update_sync_state(
        pool,
        &connection.id,
        final_sync_token.as_deref(),
        now,
    )
    .await?;

    Ok(stats)
}

fn meeting_id(connection_id: &str, google_event_id: &str) -> String {
    format!("{connection_id}:{google_event_id}")
}

async fn upsert_meeting(
    pool: &SqlitePool,
    connection: &CalendarConnectionRow,
    event: &GoogleEvent,
    meet_url: &MeetUrl,
) -> Result<(), CalendarError> {
    let now = db::now_unix();
    let start_at = parse_event_time(&event.id, &event.start)?;
    let end_at = parse_event_time(&event.id, &event.end)?;
    let attendees_json = event
        .attendees
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| CalendarError::Api(error.to_string()))?;

    let id = meeting_id(&connection.id, &event.id);

    sqlx::query(
        r#"
        INSERT INTO meetings (
            id, title, description, organizer_email, meet_url, start_at, end_at,
            attendees_json, calendar_etag, auto_join, user_id, calendar_connection_id,
            google_event_id, created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            description = excluded.description,
            organizer_email = excluded.organizer_email,
            meet_url = excluded.meet_url,
            start_at = excluded.start_at,
            end_at = excluded.end_at,
            attendees_json = excluded.attendees_json,
            calendar_etag = excluded.calendar_etag,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&id)
    .bind(event.summary.as_deref().unwrap_or("Untitled meeting"))
    .bind(event.description.as_deref())
    .bind(
        event
            .organizer
            .as_ref()
            .and_then(|organizer| organizer.email.as_deref()),
    )
    .bind(meet_url.as_str())
    .bind(start_at)
    .bind(end_at)
    .bind(attendees_json.as_deref())
    .bind(event.etag.as_deref())
    .bind(connection.user_id.as_deref())
    .bind(&connection.id)
    .bind(&event.id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

async fn delete_meeting(
    pool: &SqlitePool,
    connection: &CalendarConnectionRow,
    google_event_id: &str,
) -> Result<usize, sqlx::Error> {
    let id = meeting_id(&connection.id, google_event_id);
    let result = sqlx::query("DELETE FROM meetings WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() as usize)
}

fn meet_url(event: &GoogleEvent) -> Option<MeetUrl> {
    if let Some(link) = event
        .hangout_link
        .as_deref()
        .filter(|link| link.contains("meet.google.com"))
    {
        return MeetUrl::parse(link).ok();
    }

    if let Some(entry_points) = event
        .conference_data
        .as_ref()
        .and_then(|conference| conference.entry_points.as_ref())
    {
        for entry_point in entry_points {
            let is_video = entry_point.entry_point_type.as_deref() == Some("video");
            if let Some(uri) = entry_point
                .uri
                .as_deref()
                .filter(|uri| is_video && uri.contains("meet.google.com"))
            {
                return MeetUrl::parse(uri).ok();
            }
        }
    }

    event
        .description
        .as_deref()
        .and_then(extract_meet_url_from_text)
        .and_then(|url| MeetUrl::parse(&url).ok())
}

fn extract_meet_url_from_text(text: &str) -> Option<String> {
    text.split(|character: char| character.is_whitespace() || character == '<' || character == '>')
        .find(|part| part.contains("meet.google.com"))
        .map(|part| {
            part.trim_matches(|character: char| {
                matches!(character, '"' | '\'' | ')' | '(' | ',' | '.')
            })
            .to_owned()
        })
}

fn parse_event_time(event_id: &str, time: &EventDateTime) -> Result<i64, CalendarError> {
    let value = time
        .date_time
        .as_deref()
        .or(time.date.as_deref())
        .ok_or_else(|| CalendarError::InvalidDateTime {
            event_id: event_id.to_owned(),
            value: "missing dateTime/date".to_owned(),
        })?;

    if time.date_time.is_none() {
        return Err(CalendarError::InvalidDateTime {
            event_id: event_id.to_owned(),
            value: value.to_owned(),
        });
    }

    DateTime::parse_from_rfc3339(value)
        .map(|date| date.timestamp())
        .map_err(|_| CalendarError::InvalidDateTime {
            event_id: event_id.to_owned(),
            value: value.to_owned(),
        })
}
