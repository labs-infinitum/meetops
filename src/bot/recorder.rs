use std::path::{Path, PathBuf};

use chrono::Utc;
use sqlx::SqlitePool;
use tokio::time::{Duration, Instant};

use crate::{bot::chromium, config::Config, db::bot_runs};

pub async fn record(meet_url: String, config: &Config) -> anyhow::Result<()> {
    let recording_id = format!("manual-{}", Utc::now().format("%Y%m%dT%H%M%SZ"));
    let raw = record_inner(
        meet_url,
        config,
        &recording_id,
        RecordStop::Manual,
        None,
        async {},
    )
    .await?;
    tracing::info!(raw = %raw.display(), "recording finished");
    Ok(())
}

/// Join Meet, capture tab to `data/recordings/{recording_id}/raw.webm`, stop at `end_at_unix` or Ctrl+C.
pub async fn record_scheduled<Fut>(
    meet_url: String,
    config: &Config,
    recording_id: &str,
    end_at_unix: i64,
    pool: &SqlitePool,
    run_id: i64,
    after_joined: Fut,
) -> anyhow::Result<PathBuf>
where
    Fut: std::future::Future<Output = ()> + Send,
{
    record_inner(
        meet_url,
        config,
        recording_id,
        RecordStop::UntilUnix(end_at_unix),
        Some((pool, run_id)),
        after_joined,
    )
    .await
}

enum RecordStop {
    Manual,
    UntilUnix(i64),
}

async fn record_inner<Fut>(
    meet_url: String,
    config: &Config,
    recording_id: &str,
    stop: RecordStop,
    scheduled: Option<(&SqlitePool, i64)>,
    after_joined: Fut,
) -> anyhow::Result<PathBuf>
where
    Fut: std::future::Future<Output = ()> + Send,
{
    validate_meet_url(&meet_url)?;

    let recording_dir = config
        .storage
        .data_dir
        .join("recordings")
        .join(recording_id);
    tokio::fs::create_dir_all(&recording_dir).await?;
    tracing::info!(recording_dir = %recording_dir.display(), "created recording directory");

    let raw_webm_path = recording_dir.join("raw.webm");
    let _ = tokio::fs::remove_file(&raw_webm_path).await;

    let tab_capture_screenshot_dir = recording_dir.join("tab-capture-screenshots");

    let join_options = chromium::JoinOptions {
        meet_url,
        display_name: config.bot.display_name.clone(),
        chromium_path: config.bot.chromium_path.clone(),
        user_data_dir: config.bot.chrome_user_data_dir.clone(),
        admission_timeout: std::time::Duration::from_secs(config.bot.admission_timeout_secs),
        x11_display: config.bot.x11_display.clone(),
    };

    let mut session = chromium::open_joined_meeting(join_options).await?;

    after_joined.await;

    match chromium::grant_meet_capture_permissions(session.browser()).await {
        Ok(()) => {}
        Err(error) => {
            tracing::warn!(
                %error,
                "CDP Browser.setPermission failed; Meet may still show the capture prompt (policies alone are often insufficient)"
            );
        }
    }

    let chrome_pid = session.launched_chrome_pid();
    tracing::info!(
        chrome_pid = ?chrome_pid,
        "xdotool tab-capture helper will prefer this Chrome PID when set"
    );

    let browser_recording = match chromium::start_tab_recording(
        &session.page,
        &raw_webm_path,
        session.x11_display.clone(),
        config.bot.allow_button_template_png.clone(),
        tab_capture_screenshot_dir,
        chrome_pid,
    )
    .await
    {
        Ok(recording) => recording,
        Err(error) => {
            tracing::error!(
                %error,
                "failed to start browser recording; allow the tab-capture prompt (Linux: xdotool/Google Chrome), disable Brave blocks, or grant macOS Screen Recording if applicable"
            );
            session.close().await;
            return Err(error);
        }
    };

    if let Some((pool, rid)) = scheduled {
        if let Err(error) = bot_runs::mark_recording(pool, rid).await {
            tracing::error!(%error, run_id = rid, "failed to mark bot run recording");
        }
    }

    match stop {
        RecordStop::Manual => {
            tracing::info!(
                recording_dir = %recording_dir.display(),
                "recording started; press Ctrl+C to stop"
            );
        }
        RecordStop::UntilUnix(ts) => {
            tracing::info!(
                recording_dir = %recording_dir.display(),
                end_at_unix = ts,
                "recording started; stops at calendar end or Ctrl+C"
            );
        }
    }

    let mut status_tick = tokio::time::interval(Duration::from_secs(5));
    status_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let end_instant = match stop {
        RecordStop::Manual => None,
        RecordStop::UntilUnix(ts) => {
            let now = Utc::now().timestamp();
            if ts <= now {
                None
            } else {
                Some(Instant::now() + Duration::from_secs((ts - now) as u64))
            }
        }
    };

    let should_wait = match stop {
        RecordStop::Manual => true,
        RecordStop::UntilUnix(ts) => Utc::now().timestamp() < ts,
    };

    if should_wait {
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("stop recording: Ctrl+C");
                    break;
                }
                _ = async {
                    match end_instant {
                        Some(ins) => tokio::time::sleep_until(ins).await,
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    tracing::info!("stop recording: calendar end time reached");
                    break;
                }
                _ = status_tick.tick() => {
                    match chromium::recording_runtime_status(&session.page).await {
                        Ok(status) => tracing::info!(%status, "recording browser status"),
                        Err(error) => tracing::warn!(%error, "failed to read recording browser status"),
                    }
                }
            }
        }
    } else if matches!(stop, RecordStop::UntilUnix(_)) {
        tracing::warn!("calendar end already passed; finalizing recording");
    }

    tracing::info!("stopping browser recorder");
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        chromium::stop_tab_recording(&session.page),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            tracing::warn!(%error, "browser recorder stop failed; continuing with received chunks");
        }
        Err(_) => {
            tracing::warn!("browser recorder stop timed out; continuing with received chunks");
        }
    }
    if let Err(error) = browser_recording
        .finish_or_abort(std::time::Duration::from_secs(5))
        .await
    {
        tracing::warn!(%error, "recording writer did not finish cleanly; continuing with file on disk");
    }
    session.close().await;

    ensure_recording_exists(&raw_webm_path).await?;

    Ok(raw_webm_path)
}

async fn ensure_recording_exists(raw_webm_path: &Path) -> anyhow::Result<()> {
    let metadata = tokio::fs::metadata(raw_webm_path).await?;
    if metadata.len() == 0 {
        anyhow::bail!("recording produced an empty WebM file");
    }
    tracing::info!(bytes = metadata.len(), "saved WebM recording");
    Ok(())
}

fn validate_meet_url(meet_url: &str) -> anyhow::Result<()> {
    if meet_url.starts_with("https://meet.google.com/") {
        Ok(())
    } else {
        anyhow::bail!("expected a Google Meet URL like https://meet.google.com/xxx-yyyy-zzz")
    }
}
