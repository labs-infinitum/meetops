//! Shared bot recording execution for scheduled and ad-hoc runs.

use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    bot::runner,
    config::Config,
    db::{bot_runs, BotRunStatus},
    meet_url::MeetUrl,
    storage::RecordingsStore,
};

pub async fn run_claimed(
    config: Arc<Config>,
    pool: SqlitePool,
    chrome_sessions: Arc<Semaphore>,
    recordings: Arc<RecordingsStore>,
    run_id: i64,
    meet_url: MeetUrl,
    end_at: i64,
) {
    let permit = match chrome_sessions.acquire_owned().await {
        Ok(permit) => permit,
        Err(_) => {
            finalize_run_logged(
                &pool,
                run_id,
                BotRunStatus::Failed,
                Some("scheduler semaphore closed"),
                None,
                None,
            )
            .await;
            return;
        }
    };

    run_with_permit(permit, config, pool, recordings, run_id, meet_url, end_at).await;
}

pub async fn run_with_permit(
    _permit: OwnedSemaphorePermit,
    config: Arc<Config>,
    pool: SqlitePool,
    recordings: Arc<RecordingsStore>,
    run_id: i64,
    meet_url: MeetUrl,
    end_at: i64,
) {
    let now = crate::db::now_unix();
    if now >= end_at {
        finalize_run_logged(
            &pool,
            run_id,
            BotRunStatus::Failed,
            Some("meeting already ended before join could start"),
            None,
            None,
        )
        .await;
        return;
    }

    if let Err(error) = bot_runs::mark_joining(&pool, run_id).await {
        tracing::error!(%error, run_id, "failed to mark bot run joining");
        finalize_run_logged(
            &pool,
            run_id,
            BotRunStatus::Failed,
            Some("could not update run status to joining"),
            None,
            None,
        )
        .await;
        return;
    }

    let user_id = bot_runs::get_user_id(&pool, run_id)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "unknown".to_owned());
    let recording_dir = recordings.staging_dir(&user_id, run_id);
    let pool_joined = pool.clone();
    let result = runner::record_until_meeting_end(
        meet_url,
        end_at,
        &config,
        recording_dir,
        &pool,
        run_id,
        async move {
            if let Err(error) = bot_runs::mark_in_meeting(&pool_joined, run_id).await {
                tracing::error!(%error, run_id, "failed to mark bot run in_meeting");
            }
        },
    )
    .await;

    match result {
        Ok(raw_path) => match recordings
            .persist_bot_recording(&raw_path, &user_id, run_id)
            .await
        {
            Ok((stored, kind)) => {
                finalize_run_logged(
                    &pool,
                    run_id,
                    BotRunStatus::Completed,
                    None,
                    Some(stored.as_str()),
                    Some(kind.as_str()),
                )
                .await;
            }
            Err(error) => {
                let message = format!("recording saved locally but upload failed: {error}");
                finalize_run_logged(
                    &pool,
                    run_id,
                    BotRunStatus::Failed,
                    Some(&message),
                    None,
                    None,
                )
                .await;
            }
        },
        Err(error) => {
            let message = error.to_string();
            finalize_run_logged(&pool, run_id, BotRunStatus::Failed, Some(&message), None, None)
                .await;
        }
    }
}

async fn finalize_run_logged(
    pool: &SqlitePool,
    run_id: i64,
    status: BotRunStatus,
    error: Option<&str>,
    recording_path: Option<&str>,
    recording_storage: Option<&str>,
) {
    if let Err(err) = bot_runs::finalize_run(
        pool,
        run_id,
        status,
        error,
        recording_path,
        recording_storage,
    )
    .await
    {
        tracing::error!(%err, run_id, status = status.as_str(), "failed to finalize bot run");
    }
}
