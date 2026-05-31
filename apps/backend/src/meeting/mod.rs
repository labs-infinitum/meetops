use sqlx::SqlitePool;
use thiserror::Error;

use crate::{
    auth::{AuthError, AuthService},
    db::{self, calendar_connections, meetings::MeetingRow},
};

#[derive(Debug, Error)]
pub enum MeetingError {
    #[error("session not found or expired")]
    SessionNotFound,
    #[error("meeting not found")]
    NotFound,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<AuthError> for MeetingError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::SessionNotFound => Self::SessionNotFound,
            other => Self::Internal(other.into()),
        }
    }
}

impl From<sqlx::Error> for MeetingError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl MeetingError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SessionNotFound => "session_not_found",
            Self::NotFound => "not_found",
            Self::Internal(_) => "internal_error",
        }
    }
}

#[derive(Clone)]
pub struct MeetingService {
    db: SqlitePool,
    auth: AuthService,
}

#[derive(Debug, Clone)]
pub struct DashboardSummary {
    pub upcoming_7d: i32,
    pub connected_calendars: i32,
    pub completed_recordings: i32,
    pub upcoming_meetings: Vec<MeetingRow>,
}

impl MeetingService {
    pub fn new(db: SqlitePool, auth: AuthService) -> Self {
        Self { db, auth }
    }

    pub async fn list_meetings(&self, token: &str) -> Result<Vec<MeetingRow>, MeetingError> {
        let user_id = self.auth.require_user_id(token).await?;
        db::meetings::list_for_user(&self.db, &user_id)
            .await
            .map_err(Into::into)
    }

    pub async fn get_meeting(
        &self,
        token: &str,
        meeting_id: &str,
    ) -> Result<MeetingRow, MeetingError> {
        let user_id = self.auth.require_user_id(token).await?;
        db::meetings::get_for_user(&self.db, &user_id, meeting_id)
            .await?
            .ok_or(MeetingError::NotFound)
    }

    pub async fn set_auto_record(
        &self,
        token: &str,
        meeting_id: &str,
        auto_record: bool,
    ) -> Result<bool, MeetingError> {
        let user_id = self.auth.require_user_id(token).await?;
        let updated =
            db::meetings::set_auto_join_for_user(&self.db, &user_id, meeting_id, auto_record)
                .await?;
        if !updated {
            return Err(MeetingError::NotFound);
        }
        Ok(true)
    }

    pub async fn dashboard_summary(&self, token: &str) -> Result<DashboardSummary, MeetingError> {
        let user_id = self.auth.require_user_id(token).await?;
        let now = db::now_unix();
        let week_ahead = now + 7 * 24 * 60 * 60;

        let upcoming_7d =
            db::meetings::count_upcoming_within(&self.db, &user_id, now, week_ahead).await?;

        let calendars = calendar_connections::list_for_user(&self.db, &user_id).await?;

        let completed_recordings =
            db::bot_runs::count_completed_for_user(&self.db, &user_id).await?;

        let all = db::meetings::list_for_user(&self.db, &user_id).await?;
        let upcoming_meetings: Vec<MeetingRow> = all
            .into_iter()
            .filter(|meeting| meeting.start_at > now)
            .take(4)
            .collect();

        Ok(DashboardSummary {
            upcoming_7d: upcoming_7d as i32,
            connected_calendars: calendars.len() as i32,
            completed_recordings: completed_recordings as i32,
            upcoming_meetings,
        })
    }

}

pub fn row_to_proto_fields(row: &MeetingRow) -> (Vec<String>, String, String) {
    let attendees = db::meetings::parse_attendees(row.attendees_json.as_deref());
    let calendar_id = row.calendar_connection_id.clone().unwrap_or_default();
    let calendar_label = row.calendar_label.clone().unwrap_or_default();
    (attendees, calendar_id, calendar_label)
}
