use std::path::Path;

use chrono::Utc;

use crate::{bot::chromium, config::Config};

pub async fn record(meet_url: String, config: &Config) -> anyhow::Result<()> {
    validate_meet_url(&meet_url)?;

    let recording_id = format!("manual-{}", Utc::now().format("%Y%m%dT%H%M%SZ"));
    let recording_dir = config
        .storage
        .data_dir
        .join("recordings")
        .join(&recording_id);
    tokio::fs::create_dir_all(&recording_dir).await?;
    tracing::info!(recording_dir = %recording_dir.display(), "created recording directory");

    let raw_webm_path = recording_dir.join("raw.webm");
    let _ = tokio::fs::remove_file(&raw_webm_path).await;

    let join_options = chromium::JoinOptions {
        meet_url,
        display_name: config.bot.display_name.clone(),
        chromium_path: config.bot.chromium_path.clone(),
        user_data_dir: config.bot.chrome_user_data_dir.clone(),
        admission_timeout: std::time::Duration::from_secs(config.bot.admission_timeout_secs),
    };

    let session = chromium::open_joined_meeting(join_options).await?;
    let browser_recording = match chromium::start_tab_recording(&session.page, &raw_webm_path).await
    {
        Ok(recording) => recording,
        Err(error) => {
            tracing::error!(
                %error,
                "failed to start browser recording; Chrome may need macOS Screen & System Audio Recording permission"
            );
            session.close().await;
            return Err(error);
        }
    };
    tracing::info!(
        recording_dir = %recording_dir.display(),
        "recording started; press Ctrl+C to stop"
    );

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                match chromium::recording_runtime_status(&session.page).await {
                    Ok(status) => tracing::info!(%status, "recording browser status"),
                    Err(error) => tracing::warn!(%error, "failed to read recording browser status"),
                }
            }
        }
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

    tracing::info!(
        raw = %raw_webm_path.display(),
        "recording finished"
    );

    Ok(())
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
