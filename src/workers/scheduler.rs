//! Auto-record upcoming meetings shortly before `start_at` (see `Config.bot.join_lead_time_secs`).

use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    bot::runner,
    config::Config,
    db::{bot_runs, BotRunStatus},
    meet_url::MeetUrl,
    storage::RecordingsStore,
};

pub fn spawn(
    config: Arc<Config>,
    db: SqlitePool,
    chrome_sessions: Arc<Semaphore>,
    recordings: Arc<RecordingsStore>,
) {
    let poll_secs = config.bot.scheduler_poll_interval_secs.max(1);
    tokio::spawn(async move {
        loop {
            if let Err(error) = run_tick(&config, &db, &chrome_sessions, &recordings).await {
                tracing::error!(%error, "scheduler tick failed");
            }
            tokio::time::sleep(Duration::from_secs(poll_secs)).await;
        }
    });
}

async fn run_tick(
    config: &Arc<Config>,
    db: &SqlitePool,
    chrome_sessions: &Arc<Semaphore>,
    recordings: &Arc<RecordingsStore>,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().timestamp();
    let lead = config.bot.join_lead_time_secs as i64;
    let meetings = bot_runs::list_eligible_meetings(db, now, lead).await?;

    for meeting in meetings {
        let trigger_after = meeting.start_at.saturating_sub(lead);
        let Some(run_id) =
            bot_runs::try_claim_run(db, &meeting.id, meeting.start_at, trigger_after).await?
        else {
            continue;
        };

        let cfg = Arc::clone(config);
        let pool = db.clone();
        let meet_url = match MeetUrl::parse(&meeting.meet_url) {
            Ok(url) => url,
            Err(error) => {
                tracing::warn!(
                    meeting_id = %meeting.id,
                    meet_url = %meeting.meet_url,
                    %error,
                    "skipping scheduled recording for invalid Meet URL"
                );
                continue;
            }
        };
        let end_at = meeting.end_at;
        let sem = chrome_sessions.clone();
        let store = Arc::clone(recordings);

        tokio::spawn(async move {
            run_claimed(cfg, pool, sem, store, run_id, meet_url, end_at).await;
        });
    }

    Ok(())
}

async fn run_claimed(
    config: Arc<Config>,
    pool: SqlitePool,
    chrome_sessions: Arc<Semaphore>,
    recordings: Arc<RecordingsStore>,
    run_id: i64,
    meet_url: MeetUrl,
    end_at: i64,
) {
    let _permit: OwnedSemaphorePermit = match chrome_sessions.acquire_owned().await {
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
