use crate::db::DbPool;

use super::now_unix;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MeetingRow {
    pub id: String,
    pub title: String,
    pub meet_url: String,
    pub start_at: i64,
    pub end_at: i64,
    pub auto_join: bool,
    pub calendar_connection_id: Option<String>,
    pub description: Option<String>,
    pub attendees_json: Option<String>,
    pub calendar_label: Option<String>,
}

pub async fn list_for_user(pool: &DbPool, user_id: &str) -> sqlx::Result<Vec<MeetingRow>> {
    sqlx::query_as(
        r#"
        SELECT
            m.id,
            m.title,
            m.meet_url,
            m.start_at,
            m.end_at,
            m.auto_join,
            m.calendar_connection_id,
            m.description,
            m.attendees_json,
            cc.label AS calendar_label
        FROM meetings m
        LEFT JOIN calendar_connections cc ON cc.id = m.calendar_connection_id
        WHERE (
            m.user_id = $1
            OR m.calendar_connection_id IN (
                SELECT id FROM calendar_connections WHERE user_id = $1
            )
        )
        ORDER BY m.start_at ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn get_for_user(
    pool: &DbPool,
    user_id: &str,
    meeting_id: &str,
) -> sqlx::Result<Option<MeetingRow>> {
    sqlx::query_as(
        r#"
        SELECT
            m.id,
            m.title,
            m.meet_url,
            m.start_at,
            m.end_at,
            m.auto_join,
            m.calendar_connection_id,
            m.description,
            m.attendees_json,
            cc.label AS calendar_label
        FROM meetings m
        LEFT JOIN calendar_connections cc ON cc.id = m.calendar_connection_id
        WHERE m.id = $1
          AND (
            m.user_id = $2
            OR m.calendar_connection_id IN (
                SELECT id FROM calendar_connections WHERE user_id = $2
            )
          )
        "#,
    )
    .bind(meeting_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn count_upcoming_within(
    pool: &DbPool,
    user_id: &str,
    now: i64,
    until: i64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM meetings m
        WHERE (
            m.user_id = $1
            OR m.calendar_connection_id IN (
                SELECT id FROM calendar_connections WHERE user_id = $1
            )
        )
          AND m.start_at > $2
          AND m.start_at <= $3
        "#,
    )
    .bind(user_id)
    .bind(now)
    .bind(until)
    .fetch_one(pool)
    .await
}

pub async fn set_auto_join_for_user(
    pool: &DbPool,
    user_id: &str,
    meeting_id: &str,
    auto_join: bool,
) -> sqlx::Result<bool> {
    let now = now_unix();
    let result = sqlx::query(
        r#"
        UPDATE meetings
        SET auto_join = $1, updated_at = $2
        WHERE id = $3
          AND (
            user_id = $4
            OR calendar_connection_id IN (
                SELECT id FROM calendar_connections WHERE user_id = $4
            )
          )
        "#,
    )
    .bind(auto_join)
    .bind(now)
    .bind(meeting_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub fn parse_attendees(attendees_json: Option<&str>) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct Attendee {
        email: Option<String>,
    }

    attendees_json
        .and_then(|json| serde_json::from_str::<Vec<Attendee>>(json).ok())
        .map(|attendees| {
            attendees
                .into_iter()
                .filter_map(|attendee| attendee.email)
                .collect()
        })
        .unwrap_or_default()
}
