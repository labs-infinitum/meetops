//! Auto-record upcoming meetings shortly before `start_at` (see `Config.bot.join_lead_time_secs`).

use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::Semaphore;

use crate::{
    bot::active_recordings::ActiveRecordings,
    config::Config,
    db::{bot_runs, BotRunStatus},
    meet_url::MeetUrl,
    storage::RecordingsStore,
};

use super::bot_runner;

pub fn spawn(
    config: Arc<Config>,
    db: SqlitePool,
    chrome_sessions: Arc<Semaphore>,
    recordings: Arc<RecordingsStore>,
    active_recordings: Arc<ActiveRecordings>,
) {
    let poll_secs = config.bot.scheduler_poll_interval_secs.max(1);
    tokio::spawn(async move {
        loop {
            if let Err(error) =
                run_tick(&config, &db, &chrome_sessions, &recordings, &active_recordings).await
            {
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
    active_recordings: &Arc<ActiveRecordings>,
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

        tokio::spawn(async move {
            bot_runner::run_claimed(cfg, pool, sem, store, active, run_id, meet_url, end_at).await;
        });
    }

    Ok(())
}
