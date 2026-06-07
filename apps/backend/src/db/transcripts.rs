use crate::db::DbPool;

use super::models::{GeneratedNote, Transcript};
use super::now_unix;
use super::sql_bot_run_visible_to_user;

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

pub async fn list_generated_notes_for_user(
    pool: &DbPool,
    user_id: &str,
    limit: i64,
) -> sqlx::Result<Vec<GeneratedNote>> {
    let visible = sql_bot_run_visible_to_user("$1");
    let sql = format!(
        r#"
        SELECT
            t.id AS transcript_id,
            t.bot_run_id,
            br.meeting_id,
            m.title AS meeting_title,
            COALESCE(br.ended_at, br.started_at, br.created_at) AS recorded_at,
            t.status AS transcript_status,
            s.status AS summary_status,
            s.tldr,
            s.summary_md,
            t.raw_text AS raw_transcript,
            t.cleaned_text AS cleaned_transcript,
            s.action_items_json,
            t.error AS transcript_error,
            s.error AS summary_error
        FROM transcripts t
        JOIN bot_runs br ON br.id = t.bot_run_id
        JOIN meetings m ON m.id = br.meeting_id
        LEFT JOIN summaries s ON s.transcript_id = t.id
        WHERE {visible}
        ORDER BY COALESCE(br.ended_at, br.started_at, br.created_at) DESC, t.id DESC
        LIMIT $2
        "#
    );
    sqlx::query_as::<_, GeneratedNote>(&sql)
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await
}

pub async fn get_generated_note_for_user(
    pool: &DbPool,
    user_id: &str,
    transcript_id: i64,
) -> sqlx::Result<Option<GeneratedNote>> {
    let visible = sql_bot_run_visible_to_user("$2");
    let sql = format!(
        r#"
        SELECT
            t.id AS transcript_id,
            t.bot_run_id,
            br.meeting_id,
            m.title AS meeting_title,
            COALESCE(br.ended_at, br.started_at, br.created_at) AS recorded_at,
            t.status AS transcript_status,
            s.status AS summary_status,
            s.tldr,
            s.summary_md,
            t.raw_text AS raw_transcript,
            t.cleaned_text AS cleaned_transcript,
            s.action_items_json,
            t.error AS transcript_error,
            s.error AS summary_error
        FROM transcripts t
        JOIN bot_runs br ON br.id = t.bot_run_id
        JOIN meetings m ON m.id = br.meeting_id
        LEFT JOIN summaries s ON s.transcript_id = t.id
        WHERE t.id = $1 AND {visible}
        "#
    );
    sqlx::query_as::<_, GeneratedNote>(&sql)
        .bind(transcript_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
}
