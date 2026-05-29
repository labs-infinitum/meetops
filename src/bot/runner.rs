use std::path::PathBuf;

use sqlx::SqlitePool;

use crate::{
    bot::{chromium, recorder, x11_display},
    config::Config,
    meet_url::MeetUrl,
};

pub async fn join(meet_url: String, config: &Config) -> anyhow::Result<()> {
    let meet_url = MeetUrl::parse(&meet_url).map_err(|error| anyhow::anyhow!("{error}"))?;
    let x11 = x11_display::prepare(&config.bot).await?;
    let x11_display = x11.as_ref().map(|s| s.display.clone());

    chromium::join_meeting(chromium::JoinOptions::from_config(meet_url, config, x11_display)).await
}

pub async fn open_profile(config: &Config) -> anyhow::Result<()> {
    let x11 = x11_display::prepare(&config.bot).await?;
    let x11_display = x11.as_ref().map(|s| s.display.clone());

    chromium::open_profile(chromium::ProfileOptions::from_config(config, x11_display)).await
}

pub fn profile_status(config: &Config) -> anyhow::Result<()> {
    chromium::profile_status(&config.bot.chrome_user_data_dir)
}

pub async fn record(meet_url: String, config: &Config) -> anyhow::Result<()> {
    let meet_url = MeetUrl::parse(&meet_url).map_err(|error| anyhow::anyhow!("{error}"))?;
    recorder::record(meet_url, config).await
}

pub async fn record_until_meeting_end<Fut>(
    meet_url: MeetUrl,
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
