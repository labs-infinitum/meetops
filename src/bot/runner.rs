use std::path::PathBuf;
use std::time::Duration;

use sqlx::SqlitePool;

use crate::{
    bot::{chromium, recorder},
    config::Config,
};

pub async fn join(meet_url: String, config: &Config) -> anyhow::Result<()> {
    validate_meet_url(&meet_url)?;

    let options = chromium::JoinOptions {
        meet_url,
        display_name: config.bot.display_name.clone(),
        chromium_path: config.bot.chromium_path.clone(),
        user_data_dir: config.bot.chrome_user_data_dir.clone(),
        admission_timeout: Duration::from_secs(config.bot.admission_timeout_secs),
        x11_display: config.bot.x11_display.clone(),
    };

    chromium::join_meeting(options).await
}

pub async fn open_profile(config: &Config) -> anyhow::Result<()> {
    let options = chromium::ProfileOptions {
        chromium_path: config.bot.chromium_path.clone(),
        user_data_dir: config.bot.chrome_user_data_dir.clone(),
        x11_display: config.bot.x11_display.clone(),
    };

    chromium::open_profile(options).await
}

pub fn profile_status(config: &Config) -> anyhow::Result<()> {
    chromium::profile_status(&config.bot.chrome_user_data_dir)
}

pub async fn record(meet_url: String, config: &Config) -> anyhow::Result<()> {
    recorder::record(meet_url, config).await
}

/// Scheduled capture: join, record tab to WebM, stop at calendar `end_at_unix` or Ctrl+C.
pub async fn record_until_meeting_end<Fut>(
    meet_url: String,
    end_at_unix: i64,
    config: &Config,
    recording_id: String,
    pool: &SqlitePool,
    run_id: i64,
    after_joined: Fut,
) -> anyhow::Result<PathBuf>
where
    Fut: std::future::Future<Output = ()> + Send,
{
    recorder::record_scheduled(
        meet_url,
        config,
        &recording_id,
        end_at_unix,
        pool,
        run_id,
        after_joined,
    )
    .await
}

fn validate_meet_url(meet_url: &str) -> anyhow::Result<()> {
    if meet_url.starts_with("https://meet.google.com/") {
        Ok(())
    } else {
        anyhow::bail!("expected a Google Meet URL like https://meet.google.com/xxx-yyyy-zzz")
    }
}

#[cfg(test)]
mod tests {
    use super::validate_meet_url;

    #[test]
    fn accepts_google_meet_urls() {
        assert!(validate_meet_url("https://meet.google.com/abc-defg-hij").is_ok());
    }

    #[test]
    fn rejects_non_meet_urls() {
        assert!(validate_meet_url("https://example.com/abc-defg-hij").is_err());
    }
}
