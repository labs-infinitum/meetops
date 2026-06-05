//! Auto-record upcoming meetings shortly before `start_at` (see `Config.bot.join_lead_time_secs`).

use std::sync::Arc;
use std::time::Duration;

use crate::db::DbPool;
use tokio::sync::Semaphore;

use crate::{
    bot::{active_recordings::ActiveRecordings, recording_tasks::RecordingTasks},
    config::Config,
    db::{BotRunStatus, bot_runs},
    meet_url::MeetUrl,
    shutdown::AppShutdown,
    storage::RecordingsStore,
};

use super::bot_runner;

pub fn spawn(
    config: Arc<Config>,
    db: DbPool,
    chrome_sessions: Arc<Semaphore>,
    recordings: Arc<RecordingsStore>,
    active_recordings: Arc<ActiveRecordings>,
    recording_tasks: Arc<RecordingTasks>,
    shutdown: Arc<AppShutdown>,
) {
    let poll_secs = config.bot.scheduler_poll_interval_secs.max(1);
    let mut cancelled = shutdown.cancelled();
    tokio::spawn(async move {
        loop {
            if *cancelled.borrow() {
                tracing::info!("scheduler stopped for shutdown");
                break;
            }

            if let Err(error) = run_tick(
                &config,
                &db,
                &chrome_sessions,
                &recordings,
                &active_recordings,
                &recording_tasks,
                &shutdown,
            )
            .await
            {
                tracing::error!(%error, "scheduler tick failed");
            }

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(poll_secs)) => {}
                changed = cancelled.changed() => {
                    if changed.is_ok() && *cancelled.borrow() {
                        tracing::info!("scheduler stopped for shutdown");
                        break;
                    }
                }
            }
        }
    });
}

async fn run_tick(
    config: &Arc<Config>,
    db: &DbPool,
    chrome_sessions: &Arc<Semaphore>,
    recordings: &Arc<RecordingsStore>,
    active_recordings: &Arc<ActiveRecordings>,
    recording_tasks: &Arc<RecordingTasks>,
    shutdown: &Arc<AppShutdown>,
) -> anyhow::Result<()> {
    if shutdown.is_cancelled() {
        return Ok(());
    }

    let now = chrono::Utc::now().timestamp();
    let lead = config.bot.join_lead_time_secs as i64;
    let meetings = bot_runs::list_eligible_meetings(db, now, lead).await?;

    for meeting in meetings {
        if shutdown.is_cancelled() {
            break;
        }

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
                if let Err(finalize_err) = bot_runs::finalize_run(
                    db,
                    run_id,
                    BotRunStatus::Failed,
                    Some(&error.to_string()),
                    None,
                )
                .await
                {
                    tracing::error!(
                        %finalize_err,
                        run_id,
                        "failed to finalize bot run after invalid Meet URL"
                    );
                }
                continue;
            }
        };
        let end_at = meeting.end_at;
        let sem = chrome_sessions.clone();
        let store = Arc::clone(recordings);
        let active = Arc::clone(active_recordings);
        let shutdown_task = Arc::clone(shutdown);

        recording_tasks.spawn(async move {
            bot_runner::run_claimed(
                cfg,
                pool,
                sem,
                store,
                active,
                shutdown_task,
                run_id,
                meet_url,
                end_at,
            )
            .await;
        });
    }

    Ok(())
}
