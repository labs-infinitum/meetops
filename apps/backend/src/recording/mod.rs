use std::sync::Arc;

use sqlx::SqlitePool;
use thiserror::Error;
use tokio::sync::Semaphore;

use crate::{
    auth::{AuthError, AuthService},
    bot::active_recordings::ActiveRecordings,
    config::Config,
    db::{self, bot_runs::BotRunListItem},
    meet_url::{InvalidMeetUrl, MeetUrl},
    storage::RecordingsStore,
    workers::bot_runner,
};

#[derive(Debug, Error)]
pub enum RecordingServiceError {
    #[error("session not found or expired")]
    SessionNotFound,
    #[error("recording not found")]
    NotFound,
    #[error("recording is not available for playback")]
    NotAvailable,
    #[error("invalid Google Meet URL")]
    InvalidMeetUrl,
    #[error("bot is busy recording another meeting")]
    BotBusy,
    #[error("recording is not active")]
    NotActive,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<AuthError> for RecordingServiceError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::SessionNotFound => Self::SessionNotFound,
            other => Self::Internal(other.into()),
        }
    }
}

impl From<sqlx::Error> for RecordingServiceError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl From<InvalidMeetUrl> for RecordingServiceError {
    fn from(_: InvalidMeetUrl) -> Self {
        Self::InvalidMeetUrl
    }
}

impl RecordingServiceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SessionNotFound => "session_not_found",
            Self::NotFound => "not_found",
            Self::NotAvailable => "not_available",
            Self::InvalidMeetUrl => "invalid_meet_url",
            Self::BotBusy => "bot_busy",
            Self::NotActive => "not_active",
            Self::Internal(_) => "internal_error",
        }
    }
}

#[derive(Clone)]
pub struct RecordingService {
    db: SqlitePool,
    auth: AuthService,
    store: Arc<RecordingsStore>,
    config: Arc<Config>,
    chrome_sessions: Arc<Semaphore>,
    active_recordings: Arc<ActiveRecordings>,
}

impl RecordingService {
    pub fn new(
        db: SqlitePool,
        auth: AuthService,
        store: Arc<RecordingsStore>,
        config: Arc<Config>,
        chrome_sessions: Arc<Semaphore>,
        active_recordings: Arc<ActiveRecordings>,
    ) -> Self {
        Self {
            db,
            auth,
            store,
            config,
            chrome_sessions,
            active_recordings,
        }
    }

    pub async fn list_bot_runs(
        &self,
        token: &str,
        limit: i32,
    ) -> Result<Vec<BotRunListItem>, RecordingServiceError> {
        let user_id = self.require_user_id(token).await?;
        let limit = i64::from(limit.clamp(1, 100));
        db::bot_runs::list_recent_for_user(&self.db, &user_id, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn get_bot_run(
        &self,
        token: &str,
        run_id: i64,
    ) -> Result<BotRunListItem, RecordingServiceError> {
        let user_id = self.require_user_id(token).await?;
        db::bot_runs::get_for_user(&self.db, &user_id, run_id)
            .await?
            .ok_or(RecordingServiceError::NotFound)
    }

    pub async fn playback_url(
        &self,
        token: &str,
        run_id: i64,
    ) -> Result<(String, i64), RecordingServiceError> {
        let run = self.get_bot_run(token, run_id).await?;
        if run.status != db::BotRunStatus::Completed.as_str()
            && run.status != db::BotRunStatus::Recording.as_str()
        {
            return Err(RecordingServiceError::NotAvailable);
        }
        let Some(path) = run.recording_path.as_deref() else {
            return Err(RecordingServiceError::NotAvailable);
        };
        let url = self
            .store
            .playback_url(path)
            .await
            .map_err(RecordingServiceError::Internal)?;
        let expires_at = db::now_unix() + self.store_presigned_ttl();
        Ok((url, expires_at))
    }

    pub async fn start_ad_hoc_recording(
        &self,
        token: &str,
        meet_url_raw: &str,
    ) -> Result<BotRunListItem, RecordingServiceError> {
        let user_id = self.require_user_id(token).await?;
        let meet_url = MeetUrl::parse(meet_url_raw)?;
        let title = meet_url.ad_hoc_title();
        let now = db::now_unix();
        let duration = self.config.bot.ad_hoc_recording_duration_secs as i64;
        let end_at = now + duration;

        let permit = Arc::clone(&self.chrome_sessions)
            .try_acquire_owned()
            .map_err(|_| RecordingServiceError::BotBusy)?;

        let run_id = match db::bot_runs::create_ad_hoc_run(
            &self.db,
            &user_id,
            meet_url.as_str(),
            &title,
            now,
            end_at,
        )
        .await
        {
            Ok(run_id) => run_id,
            Err(error) => {
                drop(permit);
                return Err(error.into());
            }
        };

        let run = match db::bot_runs::get_for_user(&self.db, &user_id, run_id).await? {
            Some(run) => run,
            None => {
                drop(permit);
                return Err(RecordingServiceError::Internal(anyhow::anyhow!(
                    "ad-hoc run {run_id} missing after insert"
                )));
            }
        };

        let cfg = Arc::clone(&self.config);
        let pool = self.db.clone();
        let recordings = Arc::clone(&self.store);
        let active = Arc::clone(&self.active_recordings);
        let url = meet_url.clone();

        tokio::spawn(async move {
            bot_runner::run_with_permit(permit, cfg, pool, recordings, active, run_id, url, end_at)
                .await;
        });

        Ok(run)
    }

    pub async fn stop_recording(
        &self,
        token: &str,
        run_id: i64,
    ) -> Result<(), RecordingServiceError> {
        let user_id = self.require_user_id(token).await?;
        let run = db::bot_runs::get_for_user(&self.db, &user_id, run_id)
            .await?
            .ok_or(RecordingServiceError::NotFound)?;

        if !is_stoppable_status(&run.status) {
            return Err(RecordingServiceError::NotActive);
        }

        if !self.active_recordings.request_stop(run_id) {
            return Err(RecordingServiceError::NotActive);
        }

        Ok(())
    }

    fn store_presigned_ttl(&self) -> i64 {
        self.store.presigned_url_ttl_secs() as i64
    }

    async fn require_user_id(&self, token: &str) -> Result<String, RecordingServiceError> {
        Ok(self.auth.get_session(token).await?.user.id)
    }
}

pub fn duration_secs(run: &BotRunListItem) -> i64 {
    match (run.started_at, run.ended_at) {
        (Some(started), Some(ended)) if ended >= started => ended - started,
        _ => 0,
    }
}

fn is_stoppable_status(status: &str) -> bool {
    matches!(status, "queued" | "joining" | "in_meeting" | "recording")
}
