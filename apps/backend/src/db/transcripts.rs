use crate::db::DbPool;

use super::models::Transcript;
use super::now_unix;

const TRANSCRIPT_SELECT: &str = r#"
    SELECT
        id, bot_run_id, status, text, raw_text, cleaned_text,
        segments_json, cleaned_segments_json, language, error, created_at, updated_at
    FROM transcripts
"#;

pub async fn upsert_processing(pool: &DbPool, bot_run_id: i64) -> sqlx::Result<Transcript> {
    let now = now_unix();
    sqlx::query_as::<_, Transcript>(&format!(
        r#"
        INSERT INTO transcripts (
            bot_run_id, status, text, raw_text, cleaned_text, segments_json,
            cleaned_segments_json, language, error, created_at, updated_at
        )
        VALUES ($1, 'processing', NULL, NULL, NULL, NULL, NULL, NULL, NULL, $2, $2)
        ON CONFLICT (bot_run_id) DO UPDATE SET
            status = 'processing',
            error = NULL,
            updated_at = excluded.updated_at
        RETURNING id, bot_run_id, status, text, raw_text, cleaned_text,
            segments_json, cleaned_segments_json, language, error, created_at, updated_at
        "#
    ))
    .bind(bot_run_id)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn save_raw(
    pool: &DbPool,
    bot_run_id: i64,
    raw_text: &str,
    segments_json: Option<&str>,
    language: Option<&str>,
) -> sqlx::Result<Transcript> {
    let now = now_unix();
    sqlx::query_as::<_, Transcript>(&format!(
        r#"
        INSERT INTO transcripts (
            bot_run_id, status, text, raw_text, cleaned_text, segments_json,
            cleaned_segments_json, language, error, created_at, updated_at
        )
        VALUES ($1, 'transcribed', $2, $2, NULL, $3, NULL, $4, NULL, $5, $5)
        ON CONFLICT (bot_run_id) DO UPDATE SET
            status = 'transcribed',
            text = excluded.text,
            raw_text = excluded.raw_text,
            segments_json = excluded.segments_json,
            language = excluded.language,
            error = NULL,
            updated_at = excluded.updated_at
        RETURNING id, bot_run_id, status, text, raw_text, cleaned_text,
            segments_json, cleaned_segments_json, language, error, created_at, updated_at
        "#
    ))
    .bind(bot_run_id)
    .bind(raw_text)
    .bind(segments_json)
    .bind(language)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn save_cleaned(
    pool: &DbPool,
    transcript_id: i64,
    cleaned_text: &str,
    cleaned_segments_json: Option<&str>,
) -> sqlx::Result<Transcript> {
    let now = now_unix();
    sqlx::query_as::<_, Transcript>(&format!(
        r#"
        UPDATE transcripts
        SET status = 'cleaned',
            text = $1,
            cleaned_text = $1,
            cleaned_segments_json = $2,
            error = NULL,
            updated_at = $3
        WHERE id = $4
        RETURNING id, bot_run_id, status, text, raw_text, cleaned_text,
            segments_json, cleaned_segments_json, language, error, created_at, updated_at
        "#
    ))
    .bind(cleaned_text)
    .bind(cleaned_segments_json)
    .bind(now)
    .bind(transcript_id)
    .fetch_one(pool)
    .await
}

pub async fn mark_failed(pool: &DbPool, transcript_id: i64, error: &str) -> sqlx::Result<()> {
    let now = now_unix();
    sqlx::query(
        r#"
        UPDATE transcripts
        SET status = 'failed', error = $1, updated_at = $2
        WHERE id = $3
        "#,
    )
    .bind(error)
    .bind(now)
    .bind(transcript_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_by_id(pool: &DbPool, transcript_id: i64) -> sqlx::Result<Option<Transcript>> {
    sqlx::query_as::<_, Transcript>(&format!("{TRANSCRIPT_SELECT} WHERE id = $1"))
        .bind(transcript_id)
        .fetch_optional(pool)
        .await
}

pub async fn get_by_bot_run(pool: &DbPool, bot_run_id: i64) -> sqlx::Result<Option<Transcript>> {
    sqlx::query_as::<_, Transcript>(&format!("{TRANSCRIPT_SELECT} WHERE bot_run_id = $1"))
        .bind(bot_run_id)
        .fetch_optional(pool)
        .await
}
