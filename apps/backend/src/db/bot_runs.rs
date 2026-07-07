//! Scheduled auto-record runs and UI lifecycle.

use crate::db::DbPool;

use super::{now_unix, sql_bot_run_visible_to_user};

pub use super::models::{BotRunListItem, BotRunStatus, EligibleMeeting};

const BOT_RUN_LIST_SELECT: &str = r#"
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
"#;

/// Meetings that should auto-record: inside the trigger window, no `bot_runs` row yet for this occurrence.
pub async fn list_eligible_meetings(
    pool: &DbPool,
    now: i64,
    join_lead_secs: i64,
) -> sqlx::Result<Vec<EligibleMeeting>> {
    sqlx::query_as::<_, EligibleMeeting>(
        r#"
        SELECT m.id, m.meet_url, m.start_at, m.end_at
        FROM meetings m
        WHERE m.auto_join
          AND (m.start_at - $1) <= $2
          AND m.end_at > $2
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
    .fetch_all(pool)
    .await
}

/// Returns the new row id if this worker claimed the run, or `None` if another tick already did.
pub async fn try_claim_run(
    pool: &DbPool,
    meeting_id: &str,
    meeting_start_at: i64,
    trigger_after: i64,
) -> sqlx::Result<Option<i64>> {
    let now = now_unix();
    let result = sqlx::query(
        r#"
        INSERT INTO bot_runs (
            meeting_id, meeting_start_at, status, error, trigger_after,
            started_at, ended_at, user_id, created_at, updated_at
        )
        SELECT $1, $2, $3, NULL, $4, NULL, NULL, m.user_id, $5, $5
        FROM meetings m
        WHERE m.id = $6 AND m.user_id IS NOT NULL
        ON CONFLICT (meeting_id, meeting_start_at) DO NOTHING
        "#,
    )
    .bind(meeting_id)
    .bind(meeting_start_at)
    .bind(BotRunStatus::Queued.as_str())
    .bind(trigger_after)
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
        WHERE meeting_id = $1 AND meeting_start_at = $2
        "#,
    )
    .bind(meeting_id)
    .bind(meeting_start_at)
    .fetch_one(pool)
    .await?;

    Ok(Some(id))
}

pub async fn create_ad_hoc_run(
    pool: &DbPool,
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
        ) VALUES ($1, $2, $3, $4, $5, FALSE, $6, $7, $7)
        "#,
    )
    .bind(&meeting_id)
    .bind(title)
    .bind(meet_url)
    .bind(start_at)
    .bind(end_at)
    .bind(user_id)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    let run_id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO bot_runs (
            meeting_id, meeting_start_at, status, error, trigger_after,
            started_at, ended_at, user_id, created_at, updated_at
        ) VALUES ($1, $2, $3, NULL, $4, NULL, NULL, $5, $6, $6)
        RETURNING id
        "#,
    )
    .bind(&meeting_id)
    .bind(start_at)
    .bind(BotRunStatus::Queued.as_str())
    .bind(start_at)
    .bind(user_id)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(run_id)
}

async fn set_run_status(
    pool: &DbPool,
    run_id: i64,
    status: BotRunStatus,
    set_started_at: bool,
) -> sqlx::Result<()> {
    let now = now_unix();
    let sql = if set_started_at {
        r#"
        UPDATE bot_runs
        SET status = $1, started_at = $2, updated_at = $2
        WHERE id = $3
        "#
    } else {
        r#"
        UPDATE bot_runs
        SET status = $1, updated_at = $2
        WHERE id = $3
        "#
    };
    sqlx::query(sql)
        .bind(status.as_str())
        .bind(now)
        .bind(run_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_joining(pool: &DbPool, run_id: i64) -> sqlx::Result<()> {
    set_run_status(pool, run_id, BotRunStatus::Joining, true).await
}

pub async fn mark_in_meeting(pool: &DbPool, run_id: i64) -> sqlx::Result<()> {
    set_run_status(pool, run_id, BotRunStatus::InMeeting, false).await
}

pub async fn mark_recording(pool: &DbPool, run_id: i64) -> sqlx::Result<()> {
    set_run_status(pool, run_id, BotRunStatus::Recording, false).await
}

pub async fn get_user_id(pool: &DbPool, run_id: i64) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT user_id FROM bot_runs WHERE id = $1")
        .bind(run_id)
        .fetch_optional(pool)
        .await
}

pub async fn finalize_run(
    pool: &DbPool,
    run_id: i64,
    status: BotRunStatus,
    error: Option<&str>,
    recording_path: Option<&str>,
) -> sqlx::Result<()> {
    let now = now_unix();
    sqlx::query(
        r#"
        UPDATE bot_runs
        SET status = $1, error = $2, ended_at = $3, updated_at = $3, recording_path = $4
        WHERE id = $5
        "#,
    )
    .bind(status.as_str())
    .bind(error)
    .bind(now)
    .bind(recording_path)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Startup reconciliation: any non-terminal run from a previous process is
/// dead (its task no longer exists), so mark it failed instead of leaving it
/// stuck in `queued`/`joining`/`in_meeting`/`recording` forever.
pub async fn fail_abandoned_runs(pool: &DbPool) -> sqlx::Result<u64> {
    let now = now_unix();
    let result = sqlx::query(
        r#"
        UPDATE bot_runs
        SET status = $1,
            error = 'server restarted while this run was in progress',
            ended_at = $2,
            updated_at = $2
        WHERE status IN ('queued', 'joining', 'in_meeting', 'recording')
        "#,
    )
    .bind(BotRunStatus::Failed.as_str())
    .bind(now)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn list_recent_for_user(
    pool: &DbPool,
    user_id: &str,
    limit: i64,
) -> sqlx::Result<Vec<BotRunListItem>> {
    let visible = sql_bot_run_visible_to_user("$1");
    let sql = format!(
        "{BOT_RUN_LIST_SELECT}
        WHERE {visible}
        ORDER BY br.created_at DESC
        LIMIT $2
        "
    );
    sqlx::query_as::<_, BotRunListItem>(&sql)
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await
}

pub async fn count_completed_for_user(pool: &DbPool, user_id: &str) -> sqlx::Result<i64> {
    let visible = sql_bot_run_visible_to_user("$1");
    let sql = format!(
        r#"
        SELECT COUNT(*)
        FROM bot_runs br
        JOIN meetings m ON m.id = br.meeting_id
        WHERE br.status = 'completed' AND {visible}
        "#
    );
    sqlx::query_scalar(&sql).bind(user_id).fetch_one(pool).await
}

pub async fn get_for_user(
    pool: &DbPool,
    user_id: &str,
    run_id: i64,
) -> sqlx::Result<Option<BotRunListItem>> {
    let visible = sql_bot_run_visible_to_user("$2");
    let sql = format!(
        "{BOT_RUN_LIST_SELECT}
        WHERE br.id = $1 AND {visible}
        "
    );
    sqlx::query_as::<_, BotRunListItem>(&sql)
        .bind(run_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
}
