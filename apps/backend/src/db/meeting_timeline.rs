use sqlx::SqlitePool;

use super::models::{MeetingTimelineEvent, NewMeetingTimelineEvent};
use super::now_unix;

pub async fn insert_many(
    pool: &SqlitePool,
    bot_run_id: i64,
    events: &[NewMeetingTimelineEvent],
) -> sqlx::Result<()> {
    if events.is_empty() {
        return Ok(());
    }

    let now = now_unix();
    let mut tx = pool.begin().await?;
    for event in events {
        sqlx::query(
            r#"
            INSERT INTO meeting_timeline_events (
                bot_run_id, event_type, occurred_at, relative_ms,
                participant_id, participant_name, payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(bot_run_id)
        .bind(&event.event_type)
        .bind(event.occurred_at)
        .bind(event.relative_ms)
        .bind(&event.participant_id)
        .bind(&event.participant_name)
        .bind(&event.payload_json)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub async fn list_for_user_run(
    pool: &SqlitePool,
    user_id: &str,
    bot_run_id: i64,
    limit: i64,
) -> sqlx::Result<Vec<MeetingTimelineEvent>> {
    sqlx::query_as::<_, MeetingTimelineEvent>(
        r#"
        SELECT
            e.id,
            e.bot_run_id,
            e.event_type,
            e.occurred_at,
            e.relative_ms,
            e.participant_id,
            e.participant_name,
            e.payload_json,
            e.created_at
        FROM meeting_timeline_events e
        JOIN bot_runs br ON br.id = e.bot_run_id
        JOIN meetings m ON m.id = br.meeting_id
        WHERE e.bot_run_id = ?
          AND (
            br.user_id = ?
            OR m.user_id = ?
            OR m.calendar_connection_id IN (
                SELECT id FROM calendar_connections WHERE user_id = ?
            )
          )
        ORDER BY e.occurred_at ASC, e.id ASC
        LIMIT ?
        "#,
    )
    .bind(bot_run_id)
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}
