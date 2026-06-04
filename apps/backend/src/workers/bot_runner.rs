//! Shared bot recording execution for scheduled and ad-hoc runs.

use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    bot::{active_recordings::ActiveRecordings, runner},
    config::Config,
    db::{BotRunStatus, bot_runs},
    meet_url::MeetUrl,
    shutdown::AppShutdown,
    storage::RecordingsStore,
};

pub async fn run_claimed(
    config: Arc<Config>,
    pool: SqlitePool,
    chrome_sessions: Arc<Semaphore>,
    recordings: Arc<RecordingsStore>,
    active_recordings: Arc<ActiveRecordings>,
    shutdown: Arc<AppShutdown>,
    run_id: i64,
    meet_url: MeetUrl,
    end_at: i64,
) {
    let permit = match acquire_or_shutdown(chrome_sessions, &shutdown).await {
        Ok(permit) => permit,
        Err(()) => {
            finalize_run_logged(
                &pool,
                run_id,
                BotRunStatus::Failed,
                Some("server shutting down before recording could start"),
                None,
            )
            .await;
            return;
        }
    };

    run_with_permit(
        permit,
        config,
        pool,
        recordings,
        active_recordings,
        shutdown,
        run_id,
        meet_url,
        end_at,
    )
    .await;
}

pub async fn run_with_permit(
    permit: OwnedSemaphorePermit,
    config: Arc<Config>,
    pool: SqlitePool,
    recordings: Arc<RecordingsStore>,
    active_recordings: Arc<ActiveRecordings>,
    shutdown: Arc<AppShutdown>,
    run_id: i64,
    meet_url: MeetUrl,
    end_at: i64,
) {
    if shutdown.is_cancelled() {
        drop(permit);
        finalize_run_logged(
            &pool,
            run_id,
            BotRunStatus::Failed,
            Some("server shutting down before recording could start"),
            None,
        )
        .await;
        return;
    }

    let stop_rx = active_recordings.register(run_id);

    let result = run_with_permit_inner(
        &config,
        &pool,
        &recordings,
        run_id,
        meet_url,
        end_at,
        stop_rx,
        &shutdown,
    )
    .await;

    active_recordings.unregister(run_id);

    match result {
        Ok(raw_path) => match require_user_id(&pool, run_id).await {
            Ok(user_id) => match recordings
                .persist_bot_recording(&raw_path, &user_id, run_id)
                .await
            {
                Ok(stored) => {
                    finalize_run_logged(
                        &pool,
                        run_id,
                        BotRunStatus::Completed,
                        None,
                        Some(stored.as_str()),
                    )
                    .await;
                }
                Err(error) => {
                    let message = format!("S3 upload failed: {error}");
                    finalize_run_logged(&pool, run_id, BotRunStatus::Failed, Some(&message), None)
                        .await;
                }
            },
            Err(error) => {
                tracing::error!(%error, run_id, "cannot upload recording without user_id");
                finalize_run_logged(
                    &pool,
                    run_id,
                    BotRunStatus::Failed,
                    Some(&error.to_string()),
                    None,
                )
                .await;
            }
        },
        Err(error) => {
            let message = error.to_string();
            if let Ok(user_id) = bot_runs::get_user_id(&pool, run_id).await {
                if let Some(user_id) = user_id {
                    let staging_dir = recordings.staging_dir(&user_id, run_id);
                    if let Err(upload_err) = recordings
                        .persist_tab_capture_if_present(&staging_dir, &user_id, run_id)
                        .await
                    {
                        tracing::warn!(
                            %upload_err,
                            run_id,
                            "failed to upload tab-capture.png after bot run failure"
                        );
                    }
                }
            }
            finalize_run_logged(&pool, run_id, BotRunStatus::Failed, Some(&message), None).await;
        }
    }
}

async fn run_with_permit_inner(
    config: &Arc<Config>,
    pool: &SqlitePool,
    recordings: &Arc<RecordingsStore>,
    run_id: i64,
    meet_url: MeetUrl,
    end_at: i64,
    stop_rx: tokio::sync::watch::Receiver<bool>,
    shutdown: &AppShutdown,
) -> anyhow::Result<std::path::PathBuf> {
    let now = crate::db::now_unix();
    if now >= end_at {
        anyhow::bail!("meeting already ended before join could start");
    }

    if let Err(error) = bot_runs::mark_joining(pool, run_id).await {
        tracing::error!(%error, run_id, "failed to mark bot run joining");
        anyhow::bail!("could not update run status to joining");
    }

    let user_id = require_user_id(pool, run_id).await?;
    let recording_dir = recordings.staging_dir(&user_id, run_id);
    let pool_joined = pool.clone();

    runner::record_until_meeting_end(
        meet_url,
        end_at,
        config,
        recording_dir,
        pool,
        run_id,
        stop_rx,
        shutdown,
        async move {
            if let Err(error) = bot_runs::mark_in_meeting(&pool_joined, run_id).await {
                tracing::error!(%error, run_id, "failed to mark bot run in_meeting");
            }
        },
    )
    .await
}

async fn require_user_id(pool: &SqlitePool, run_id: i64) -> anyhow::Result<String> {
    let user_id = bot_runs::get_user_id(pool, run_id)
        .await
        .map_err(|error| anyhow::anyhow!("failed to load user_id for bot run {run_id}: {error}"))?;
    user_id.ok_or_else(|| anyhow::anyhow!("bot run {run_id} has no user_id"))
}

async fn finalize_run_logged(
    pool: &SqlitePool,
    run_id: i64,
    status: BotRunStatus,
    error: Option<&str>,
    recording_path: Option<&str>,
) {
    if let Err(err) = bot_runs::finalize_run(pool, run_id, status, error, recording_path).await {
        tracing::error!(%err, run_id, status = status.as_str(), "failed to finalize bot run");
    }
}

async fn acquire_or_shutdown(
    chrome_sessions: Arc<Semaphore>,
    shutdown: &AppShutdown,
) -> Result<OwnedSemaphorePermit, ()> {
    loop {
        if shutdown.is_cancelled() {
            return Err(());
        }

        let sem = Arc::clone(&chrome_sessions);
        let mut cancelled = shutdown.cancelled();
        tokio::select! {
            permit = sem.acquire_owned() => return permit.map_err(|_| ()),
            changed = cancelled.changed() => {
                if *cancelled.borrow() {
                    return Err(());
                }
                if changed.is_err() {
                    return Arc::clone(&chrome_sessions)
                        .acquire_owned()
                        .await
                        .map_err(|_| ());
                }
            }
        }
    }
}
