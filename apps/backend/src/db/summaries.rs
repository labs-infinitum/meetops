use crate::db::DbPool;

use super::models::Summary;
use super::now_unix;

pub async fn upsert_processing(pool: &DbPool, transcript_id: i64) -> sqlx::Result<Summary> {
    let now = now_unix();
    sqlx::query_as::<_, Summary>(
        r#"
        INSERT INTO summaries (
            transcript_id, status, tldr, summary_md, action_items_json, error, created_at, updated_at
        )
        VALUES ($1, 'processing', NULL, NULL, NULL, NULL, $2, $2)
        ON CONFLICT (transcript_id) DO UPDATE SET
            status = 'processing',
            error = NULL,
            updated_at = excluded.updated_at
        RETURNING id, transcript_id, status, tldr, summary_md, action_items_json, error, created_at, updated_at
        "#,
    )
    .bind(transcript_id)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn save_completed(
    pool: &DbPool,
    transcript_id: i64,
    tldr: &str,
    summary_md: &str,
    action_items_json: &str,
) -> sqlx::Result<Summary> {
    let now = now_unix();
    sqlx::query_as::<_, Summary>(
        r#"
        INSERT INTO summaries (
            transcript_id, status, tldr, summary_md, action_items_json, error, created_at, updated_at
        )
        VALUES ($1, 'completed', $2, $3, $4, NULL, $5, $5)
        ON CONFLICT (transcript_id) DO UPDATE SET
            status = 'completed',
            tldr = excluded.tldr,
            summary_md = excluded.summary_md,
            action_items_json = excluded.action_items_json,
            error = NULL,
            updated_at = excluded.updated_at
        RETURNING id, transcript_id, status, tldr, summary_md, action_items_json, error, created_at, updated_at
        "#,
    )
    .bind(transcript_id)
    .bind(tldr)
    .bind(summary_md)
    .bind(action_items_json)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn mark_failed(pool: &DbPool, transcript_id: i64, error: &str) -> sqlx::Result<()> {
    let now = now_unix();
    sqlx::query(
        r#"
        UPDATE summaries
        SET status = 'failed', error = $1, updated_at = $2
        WHERE transcript_id = $3
        "#,
    )
    .bind(error)
    .bind(now)
    .bind(transcript_id)
    .execute(pool)
    .await?;
    Ok(())
}
