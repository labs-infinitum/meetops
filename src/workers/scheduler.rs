//! Auto-record upcoming meetings shortly before `start_at` (see `Config.bot.join_lead_time_secs`).

use std::{path::Path, sync::Arc, time::Duration};

use sqlx::SqlitePool;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    bot::runner,
    config::Config,
    db::{bot_runs, BotRunStatus},
    meet_url::MeetUrl,
};

pub fn spawn(config: Arc<Config>, db: SqlitePool, chrome_sessions: Arc<Semaphore>) {
    let poll_secs = config.bot.scheduler_poll_interval_secs.max(1);
    tokio::spawn(async move {
        loop {
            if let Err(error) = run_tick(&config, &db, &chrome_sessions).await {
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

        tokio::spawn(async move {
            run_claimed(cfg, pool, sem, run_id, meet_url, end_at).await;
        });
    }

    Ok(())
}

fn recording_path_for_db(config: &Config, raw_path: &Path) -> String {
    raw_path
        .strip_prefix(&config.storage.data_dir)
        .ok()
        .and_then(|p| p.to_str().map(str::to_owned))
        .unwrap_or_else(|| raw_path.to_string_lossy().into_owned())
}

async fn run_claimed(
    config: Arc<Config>,
    pool: SqlitePool,
    chrome_sessions: Arc<Semaphore>,
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
        )
        .await;
        return;
    }

    let recording_id = format!("run-{run_id}");
    let pool_joined = pool.clone();
    let result = runner::record_until_meeting_end(
        meet_url,
        end_at,
        &config,
        recording_id,
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
        Ok(raw_path) => {
            let rel = recording_path_for_db(&config, &raw_path);
            finalize_run_logged(&pool, run_id, BotRunStatus::Completed, None, Some(rel.as_str())).await;
        }
        Err(error) => {
            let message = error.to_string();
            finalize_run_logged(&pool, run_id, BotRunStatus::Failed, Some(&message), None).await;
        }
    }
}

async fn finalize_run_logged(
    pool: &SqlitePool,
    run_id: i64,
    status: BotRunStatus,
    error: Option<&str>,
    recording_path: Option<&str>,
) {
    if let Err(err) =
        bot_runs::finalize_run(pool, run_id, status, error, recording_path).await
    {
        tracing::error!(%err, run_id, status = status.as_str(), "failed to finalize bot run");
    }
}
