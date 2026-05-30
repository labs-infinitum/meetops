//! Scheduled auto-record runs and UI lifecycle.

use sqlx::SqlitePool;

use super::now_unix;

pub use super::models::{BotRunListItem, BotRunStatus, EligibleMeeting};

/// Meetings that should auto-record: inside the trigger window, no `bot_runs` row yet for this occurrence.
pub async fn list_eligible_meetings(
    pool: &SqlitePool,
    now: i64,
    join_lead_secs: i64,
) -> sqlx::Result<Vec<EligibleMeeting>> {
    sqlx::query_as::<_, EligibleMeeting>(
        r#"
        SELECT m.id, m.meet_url, m.start_at, m.end_at
        FROM meetings m
        WHERE m.auto_join = 1
          AND (m.start_at - ?) <= ?
          AND m.end_at > ?
          AND NOT EXISTS (
            SELECT 1
            FROM bot_runs b
            WHERE b.meeting_id = m.id AND b.meeting_start_at = m.start_at
          )
        ORDER BY m.start_at ASC
        "#,
    )
    .bind(join_lead_secs)
    .bind(now)
    .bind(now)
    .fetch_all(pool)
    .await
}

/// Returns the new row id if this worker claimed the run, or `None` if another tick already did.
pub async fn try_claim_run(
    pool: &SqlitePool,
    meeting_id: &str,
    meeting_start_at: i64,
    trigger_after: i64,
) -> sqlx::Result<Option<i64>> {
    let now = now_unix();
    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO bot_runs (
            meeting_id, meeting_start_at, status, error, trigger_after,
            started_at, ended_at, user_id, created_at, updated_at
        )
        SELECT ?, ?, ?, NULL, ?, NULL, NULL, m.user_id, ?, ?
        FROM meetings m
        WHERE m.id = ? AND m.user_id IS NOT NULL
        "#,
    )
    .bind(meeting_id)
    .bind(meeting_start_at)
    .bind(BotRunStatus::Queued.as_str())
    .bind(trigger_after)
    .bind(now)
    .bind(now)
    .bind(meeting_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }

    let id = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id FROM bot_runs
        WHERE meeting_id = ? AND meeting_start_at = ?
        "#,
    )
    .bind(meeting_id)
    .bind(meeting_start_at)
    .fetch_one(pool)
    .await?;

    Ok(Some(id))
}

pub async fn create_ad_hoc_run(
    pool: &SqlitePool,
    user_id: &str,
    meet_url: &str,
    title: &str,
    start_at: i64,
    end_at: i64,
) -> sqlx::Result<i64> {
    use uuid::Uuid;

    let meeting_id = format!("adhoc:{}", Uuid::new_v4());
    let now = now_unix();

    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO meetings (
            id, title, meet_url, start_at, end_at, auto_join,
            user_id, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?)
        "#,
    )
    .bind(&meeting_id)
    .bind(title)
    .bind(meet_url)
    .bind(start_at)
    .bind(end_at)
    .bind(user_id)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    let result = sqlx::query(
        r#"
        INSERT INTO bot_runs (
            meeting_id, meeting_start_at, status, error, trigger_after,
            started_at, ended_at, user_id, created_at, updated_at
        ) VALUES (?, ?, ?, NULL, ?, NULL, NULL, ?, ?, ?)
        "#,
    )
    .bind(&meeting_id)
    .bind(start_at)
    .bind(BotRunStatus::Queued.as_str())
    .bind(start_at)
    .bind(user_id)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(result.last_insert_rowid())
}

pub async fn mark_joining(pool: &SqlitePool, run_id: i64) -> sqlx::Result<()> {
    let now = now_unix();
    sqlx::query(
        r#"
        UPDATE bot_runs
        SET status = ?, started_at = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(BotRunStatus::Joining.as_str())
    .bind(now)
    .bind(now)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_in_meeting(pool: &SqlitePool, run_id: i64) -> sqlx::Result<()> {
    let now = now_unix();
    sqlx::query(
        r#"
        UPDATE bot_runs
        SET status = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(BotRunStatus::InMeeting.as_str())
    .bind(now)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_recording(pool: &SqlitePool, run_id: i64) -> sqlx::Result<()> {
    let now = now_unix();
    sqlx::query(
        r#"
        UPDATE bot_runs
        SET status = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(BotRunStatus::Recording.as_str())
    .bind(now)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_user_id(pool: &SqlitePool, run_id: i64) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT user_id FROM bot_runs WHERE id = ?")
        .bind(run_id)
        .fetch_optional(pool)
        .await
}

pub async fn finalize_run(
    pool: &SqlitePool,
    run_id: i64,
    status: BotRunStatus,
    error: Option<&str>,
    recording_path: Option<&str>,
) -> sqlx::Result<()> {
    let now = now_unix();
    sqlx::query(
        r#"
        UPDATE bot_runs
        SET status = ?, error = ?, ended_at = ?, updated_at = ?, recording_path = ?
        WHERE id = ?
        "#,
    )
    .bind(status.as_str())
    .bind(error)
    .bind(now)
    .bind(now)
    .bind(recording_path)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_recent_for_user(
    pool: &SqlitePool,
    user_id: &str,
    limit: i64,
) -> sqlx::Result<Vec<BotRunListItem>> {
    sqlx::query_as::<_, BotRunListItem>(
        r#"
        SELECT
            br.id,
            br.meeting_id,
            m.title,
            br.status,
            br.error,
            br.meeting_start_at,
            br.started_at,
            br.ended_at,
            br.created_at,
            br.recording_path
        FROM bot_runs br
        JOIN meetings m ON m.id = br.meeting_id
        WHERE br.user_id = ?
           OR m.user_id = ?
           OR m.calendar_connection_id IN (
               SELECT id FROM calendar_connections WHERE user_id = ?
           )
        ORDER BY br.created_at DESC
        LIMIT ?
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn count_completed_for_user(pool: &SqlitePool, user_id: &str) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM bot_runs br
        JOIN meetings m ON m.id = br.meeting_id
        WHERE br.status = 'completed'
          AND (
            br.user_id = ?
            OR m.user_id = ?
            OR m.calendar_connection_id IN (
                SELECT id FROM calendar_connections WHERE user_id = ?
            )
          )
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

pub async fn get_for_user(
    pool: &SqlitePool,
    user_id: &str,
    run_id: i64,
) -> sqlx::Result<Option<BotRunListItem>> {
    sqlx::query_as::<_, BotRunListItem>(
        r#"
        SELECT
            br.id,
            br.meeting_id,
            m.title,
            br.status,
            br.error,
            br.meeting_start_at,
            br.started_at,
            br.ended_at,
            br.created_at,
            br.recording_path
        FROM bot_runs br
        JOIN meetings m ON m.id = br.meeting_id
        WHERE br.id = ?
          AND (
            br.user_id = ?
            OR m.user_id = ?
            OR m.calendar_connection_id IN (
                SELECT id FROM calendar_connections WHERE user_id = ?
            )
          )
        "#,
    )
    .bind(run_id)
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}
