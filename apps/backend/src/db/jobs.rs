use crate::db::DbPool;

use super::models::Job;
use super::now_unix;

pub const STATUS_QUEUED: &str = "queued";
pub const STATUS_RUNNING: &str = "running";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_FAILED: &str = "failed";

pub const KIND_TRANSCRIBE_RECORDING: &str = "transcribe_recording";
pub const KIND_CLEAN_TRANSCRIPT: &str = "clean_transcript";
pub const KIND_GENERATE_MEETING_NOTES: &str = "generate_meeting_notes";

pub async fn enqueue_for_bot_run(
    pool: &DbPool,
    kind: &str,
    bot_run_id: i64,
    transcript_id: Option<i64>,
    payload_json: Option<&str>,
) -> sqlx::Result<Option<i64>> {
    let now = now_unix();
    sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO jobs (
            kind, status, meeting_id, bot_run_id, transcript_id, payload_json,
            attempts, run_after, locked_at, last_error, created_at, updated_at
        )
        SELECT $1, $2, NULL, $3, $4, $5, 0, $6, NULL, NULL, $6, $6
        WHERE NOT EXISTS (
            SELECT 1 FROM jobs
            WHERE kind = $1
              AND bot_run_id = $3
              AND COALESCE(transcript_id, -1) = COALESCE($4, -1)
              AND status IN ('queued', 'running', 'completed')
        )
        RETURNING id
        "#,
    )
    .bind(kind)
    .bind(STATUS_QUEUED)
    .bind(bot_run_id)
    .bind(transcript_id)
    .bind(payload_json)
    .bind(now)
    .fetch_optional(pool)
    .await
}

pub async fn claim_next(pool: &DbPool) -> sqlx::Result<Option<Job>> {
    let now = now_unix();
    sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET status = $1,
            locked_at = $2,
            attempts = attempts + 1,
            updated_at = $2
        WHERE id = (
            SELECT id
            FROM jobs
            WHERE status = $3 AND run_after <= $2
            ORDER BY run_after ASC, id ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING
            id, kind, status, meeting_id, bot_run_id, transcript_id, payload_json,
            attempts, run_after, locked_at, last_error, created_at, updated_at
        "#,
    )
    .bind(STATUS_RUNNING)
    .bind(now)
    .bind(STATUS_QUEUED)
    .fetch_optional(pool)
    .await
}

pub async fn complete(pool: &DbPool, job_id: i64) -> sqlx::Result<()> {
    let now = now_unix();
    sqlx::query(
        r#"
        UPDATE jobs
        SET status = $1, locked_at = NULL, last_error = NULL, updated_at = $2
        WHERE id = $3
        "#,
    )
    .bind(STATUS_COMPLETED)
    .bind(now)
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fail_or_retry(
    pool: &DbPool,
    job: &Job,
    error: &str,
    max_attempts: i64,
) -> sqlx::Result<()> {
    let now = now_unix();
    let retry = job.attempts < max_attempts;
    let status = if retry { STATUS_QUEUED } else { STATUS_FAILED };
    let run_after = if retry {
        now + retry_delay_secs(job.attempts)
    } else {
        now
    };
    sqlx::query(
        r#"
        UPDATE jobs
        SET status = $1,
            run_after = $2,
            locked_at = NULL,
            last_error = $3,
            updated_at = $4
        WHERE id = $5
        "#,
    )
    .bind(status)
    .bind(run_after)
    .bind(error)
    .bind(now)
    .bind(job.id)
    .execute(pool)
    .await?;
    Ok(())
}

fn retry_delay_secs(attempts: i64) -> i64 {
    match attempts {
        0 | 1 => 30,
        2 => 120,
        3 => 300,
        _ => 900,
    }
}

#[cfg(test)]
mod tests {
    use super::retry_delay_secs;

    #[test]
    fn retry_delay_backs_off() {
        assert_eq!(retry_delay_secs(1), 30);
        assert_eq!(retry_delay_secs(2), 120);
        assert_eq!(retry_delay_secs(3), 300);
        assert_eq!(retry_delay_secs(4), 900);
    }
}
