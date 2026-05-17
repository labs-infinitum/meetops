//! Persisted state for scheduled auto-record (M4) and UI lifecycle.

use sqlx::SqlitePool;

use super::now_unix;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EligibleMeeting {
    pub id: String,
    pub meet_url: String,
    pub start_at: i64,
    pub end_at: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BotRunListItem {
    pub id: i64,
    pub meeting_id: String,
    pub title: String,
    pub status: String,
    pub error: Option<String>,
    pub meeting_start_at: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub created_at: i64,
    pub recording_path: Option<String>,
}

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
            started_at, ended_at, created_at, updated_at
        )
        VALUES (?, ?, 'queued', NULL, ?, NULL, NULL, ?, ?)
        "#,
    )
    .bind(meeting_id)
    .bind(meeting_start_at)
    .bind(trigger_after)
    .bind(now)
    .bind(now)
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

pub async fn mark_joining(pool: &SqlitePool, run_id: i64) -> sqlx::Result<()> {
    let now = now_unix();
    sqlx::query(
        r#"
        UPDATE bot_runs
        SET status = 'joining', started_at = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
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
        SET status = 'in_meeting', updated_at = ?
        WHERE id = ?
        "#,
    )
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
        SET status = 'recording', updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(now)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn finalize_run(
    pool: &SqlitePool,
    run_id: i64,
    status: &str,
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
    .bind(status)
    .bind(error)
    .bind(now)
    .bind(now)
    .bind(recording_path)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_recent(pool: &SqlitePool, limit: i64) -> sqlx::Result<Vec<BotRunListItem>> {
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
        ORDER BY br.created_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn set_meeting_auto_join(
    pool: &SqlitePool,
    meeting_id: &str,
    auto_join: bool,
) -> sqlx::Result<u64> {
    let now = now_unix();
    let flag = if auto_join { 1 } else { 0 };
    let result = sqlx::query(
        r#"
        UPDATE meetings
        SET auto_join = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(flag)
    .bind(now)
    .bind(meeting_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
