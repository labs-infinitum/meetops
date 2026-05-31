use std::path::{Path, PathBuf};

use chrono::Utc;
use sqlx::SqlitePool;
use tokio::sync::watch;
use tokio::time::{Duration, Instant};

use crate::{
    bot::{chromium, x11_display},
    config::Config,
    db::bot_runs,
    meet_url::MeetUrl,
};

pub async fn record(
    meet_url: MeetUrl,
    config: &Config,
    recording_dir: PathBuf,
) -> anyhow::Result<PathBuf> {
    let (_tx, stop_rx) = watch::channel(false);
    let raw = record_inner(
        meet_url,
        config,
        recording_dir,
        RecordStop::Manual,
        None,
        stop_rx,
        async {},
    )
    .await?;
    tracing::info!(raw = %raw.display(), "recording finished");
    Ok(raw)
}

/// Join Meet, capture tab to `{recording_dir}/raw.webm`, stop at `end_at_unix` or Ctrl+C.
pub async fn record_scheduled<Fut>(
    meet_url: MeetUrl,
    config: &Config,
    recording_dir: PathBuf,
    end_at_unix: i64,
    pool: &SqlitePool,
    run_id: i64,
    stop_rx: watch::Receiver<bool>,
    after_joined: Fut,
) -> anyhow::Result<PathBuf>
where
    Fut: std::future::Future<Output = ()> + Send,
{
    record_inner(
        meet_url,
        config,
        recording_dir,
        RecordStop::UntilUnix(end_at_unix),
        Some((pool, run_id)),
        stop_rx,
        after_joined,
    )
    .await
}

enum RecordStop {
    Manual,
    UntilUnix(i64),
}

enum RecordExitReason {
    CtrlC,
    CalendarEnd,
    UserStop,
    SoloParticipant,
}

async fn record_inner<Fut>(
    meet_url: MeetUrl,
    config: &Config,
    recording_dir: PathBuf,
    stop: RecordStop,
    scheduled: Option<(&SqlitePool, i64)>,
    mut stop_rx: watch::Receiver<bool>,
    after_joined: Fut,
) -> anyhow::Result<PathBuf>
where
    Fut: std::future::Future<Output = ()> + Send,
{
    let x11 = x11_display::prepare(&config.bot).await?;
    let x11_display = x11.as_ref().map(|s| s.display.clone());

    tokio::fs::create_dir_all(&recording_dir).await?;
    tracing::info!(recording_dir = %recording_dir.display(), "created recording directory");

    let raw_webm_path = recording_dir.join("raw.webm");
    let audio_webm_path = recording_dir.join("audio.webm");
    if let Err(error) = tokio::fs::remove_file(&raw_webm_path).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(error.into());
    }
    if let Err(error) = tokio::fs::remove_file(&audio_webm_path).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(error.into());
    }

    let tab_capture_screenshot_dir = recording_dir.join("tab-capture-screenshots");
    let tab_capture_png = recording_dir.join("tab-capture.png");
    let _ = tokio::fs::remove_file(&tab_capture_png).await;

    let join_options = chromium::JoinOptions::from_config(meet_url, config, x11_display.clone());

    let mut session = chromium::open_joined_meeting(join_options).await?;

    if *stop_rx.borrow() {
        session.close().await;
        anyhow::bail!("stopped by user");
    }

    after_joined.await;

    if *stop_rx.borrow() {
        session.close().await;
        anyhow::bail!("stopped by user");
    }

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
        &audio_webm_path,
        scheduled.map(|(pool, run_id)| (pool.clone(), run_id)),
        session.x11_display.clone(),
        tab_capture_screenshot_dir,
        tab_capture_png,
        chrome_pid,
    )
    .await
    {
        Ok(recording) => recording,
        Err(error) => {
            tracing::error!(
                %error,
                "failed to start browser recording; allow the tab-capture prompt (Linux: xdotool/Google Chrome) or grant macOS Screen Recording if applicable"
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
                "recording started; stops at calendar end, solo timeout, user stop, or Ctrl+C"
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

    let solo_threshold = Duration::from_secs(config.bot.solo_participant_leave_secs.max(1));
    let mut solo_since: Option<Instant> = None;
    let mut exit_reason: Option<RecordExitReason> = None;

    if should_wait {
        loop {
            if *stop_rx.borrow() {
                exit_reason = Some(RecordExitReason::UserStop);
                break;
            }

            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    exit_reason = Some(RecordExitReason::CtrlC);
                    break;
                }
                _ = async {
                    match end_instant {
                        Some(ins) => tokio::time::sleep_until(ins).await,
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    exit_reason = Some(RecordExitReason::CalendarEnd);
                    break;
                }
                changed = stop_rx.changed() => {
                    if changed.is_ok() && *stop_rx.borrow() {
                        exit_reason = Some(RecordExitReason::UserStop);
                        break;
                    }
                }
                _ = status_tick.tick() => {
                    chromium::dismiss_meet_overlays_best_effort(&session.page).await;

                    match chromium::recording_runtime_status(&session.page).await {
                        Ok(status) => tracing::info!(%status, "recording browser status"),
                        Err(error) => tracing::warn!(%error, "failed to read recording browser status"),
                    }

                    match chromium::meet_participant_count(&session.page).await {
                        Ok(Some(count)) if count <= 1 => {
                            let since = solo_since.get_or_insert_with(Instant::now);
                            if since.elapsed() >= solo_threshold {
                                exit_reason = Some(RecordExitReason::SoloParticipant);
                                break;
                            }
                        }
                        Ok(Some(_)) => {
                            solo_since = None;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            tracing::warn!(%error, "failed to read Meet participant count");
                        }
                    }
                }
            }
        }
    } else if matches!(stop, RecordStop::UntilUnix(_)) {
        tracing::warn!("calendar end already passed; finalizing recording");
    }

    match exit_reason {
        Some(RecordExitReason::CtrlC) => tracing::info!("stop recording: Ctrl+C"),
        Some(RecordExitReason::CalendarEnd) => {
            tracing::info!("stop recording: calendar end time reached")
        }
        Some(RecordExitReason::UserStop) => tracing::info!("stop recording: user requested"),
        Some(RecordExitReason::SoloParticipant) => tracing::info!(
            secs = solo_threshold.as_secs(),
            "stop recording: no other participants in Meet"
        ),
        None => {}
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
