use crate::db::DbPool;

use super::models::{MeetingTimelineEvent, NewMeetingTimelineEvent};
use super::{now_unix, sql_bot_run_visible_to_user};

pub async fn insert_many(
    pool: &DbPool,
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
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
    pool: &DbPool,
    user_id: &str,
    bot_run_id: i64,
    limit: i64,
) -> sqlx::Result<Vec<MeetingTimelineEvent>> {
    let visible = sql_bot_run_visible_to_user("$2");
    let sql = format!(
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
        WHERE e.bot_run_id = $1 AND {visible}
        ORDER BY e.occurred_at ASC, e.id ASC
        LIMIT $3
        "#
    );
    sqlx::query_as::<_, MeetingTimelineEvent>(&sql)
        .bind(bot_run_id)
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await
}

pub async fn list_for_run(
    pool: &DbPool,
    bot_run_id: i64,
    limit: i64,
) -> sqlx::Result<Vec<MeetingTimelineEvent>> {
    sqlx::query_as::<_, MeetingTimelineEvent>(
        r#"
        SELECT
            id,
            bot_run_id,
            event_type,
            occurred_at,
            relative_ms,
            participant_id,
            participant_name,
            payload_json,
            created_at
        FROM meeting_timeline_events
        WHERE bot_run_id = $1
        ORDER BY occurred_at ASC, id ASC
        LIMIT $2
        "#,
    )
    .bind(bot_run_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}
