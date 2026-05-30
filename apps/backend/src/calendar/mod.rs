use std::sync::Arc;

use reqwest::Client;
use sqlx::SqlitePool;
use thiserror::Error;

use crate::{
    auth::{AuthError, AuthService},
    config::Config,
    db::calendar_connections::{self, CalendarConnectionListItem, CalendarConnectionRow},
    google::{calendar, oauth::{self, OAuthStatePayload}},
};

#[derive(Debug, Error)]
pub enum CalendarConnectError {
    #[error("session not found or expired")]
    SessionNotFound,
    #[error("provider not supported: {0}")]
    UnsupportedProvider(String),
    #[error("calendar connection not found")]
    NotFound,
    #[error("already connected by another user")]
    AlreadyConnectedByOther {
        email: String,
        provider: String,
        other_account_hint: String,
    },
    #[error(transparent)]
    GoogleAuth(#[from] oauth::GoogleAuthError),
    #[error(transparent)]
    Calendar(#[from] calendar::CalendarError),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<AuthError> for CalendarConnectError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::SessionNotFound => Self::SessionNotFound,
            other => Self::Internal(other.into()),
        }
    }
}

impl From<sqlx::Error> for CalendarConnectError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl CalendarConnectError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SessionNotFound => "session_not_found",
            Self::UnsupportedProvider(_) => "provider_unavailable",
            Self::NotFound => "not_found",
            Self::AlreadyConnectedByOther { .. } => "already_connected_by_other_user",
            Self::GoogleAuth(error) => match error {
                oauth::GoogleAuthError::MissingConfig => "provider_unavailable",
                oauth::GoogleAuthError::AccountLinkedToOtherUser { .. } => {
                    "already_connected_by_other_user"
                }
                _ => "oauth_failed",
            },
            Self::Calendar(_) => "sync_failed",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn email(&self) -> Option<&str> {
        match self {
            Self::AlreadyConnectedByOther { email, .. } => Some(email.as_str()),
            Self::GoogleAuth(oauth::GoogleAuthError::AccountLinkedToOtherUser { email }) => {
                Some(email.as_str())
            }
            _ => None,
        }
    }

    pub fn provider(&self) -> Option<&str> {
        match self {
            Self::AlreadyConnectedByOther { provider, .. } => Some(provider.as_str()),
            _ => Some("google"),
        }
    }

    pub fn other_account_hint(&self) -> Option<&str> {
        match self {
            Self::AlreadyConnectedByOther {
                other_account_hint,
                ..
            } => Some(other_account_hint.as_str()),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct CalendarService {
    db: SqlitePool,
    http: Client,
    config: Arc<Config>,
    auth: AuthService,
}

impl CalendarService {
    pub fn new(db: SqlitePool, http: Client, config: Arc<Config>, auth: AuthService) -> Self {
        Self {
            db,
            http,
            config,
            auth,
        }
    }

    pub async fn list_connections(
        &self,
        token: &str,
    ) -> Result<Vec<CalendarConnectionListItem>, CalendarConnectError> {
        let user_id = self.require_user_id(token).await?;
        calendar_connections::list_for_user(&self.db, &user_id)
            .await
            .map_err(CalendarConnectError::from)
    }

    pub async fn start_oauth(
        &self,
        token: &str,
        provider: &str,
        label: Option<&str>,
        color: Option<&str>,
    ) -> Result<String, CalendarConnectError> {
        let user_id = self.require_user_id(token).await?;

        if provider != "google" {
            return Err(CalendarConnectError::UnsupportedProvider(provider.to_owned()));
        }

        oauth::start_url(
            &self.db,
            &self.config,
            OAuthStatePayload {
                user_id,
                provider: provider.to_owned(),
                label: label.map(str::to_owned),
                color: color.map(str::to_owned),
            },
        )
        .await
        .map_err(CalendarConnectError::from)
    }

    pub async fn disconnect(
        &self,
        token: &str,
        connection_id: &str,
    ) -> Result<bool, CalendarConnectError> {
        let user_id = self.require_user_id(token).await?;
        Ok(calendar_connections::delete_for_user(&self.db, connection_id, &user_id).await?)
    }

    pub async fn sync_now(
        &self,
        token: &str,
        connection_id: &str,
    ) -> Result<(calendar::SyncStats, i64), CalendarConnectError> {
        let user_id = self.require_user_id(token).await?;
        let connection = calendar_connections::get_by_id_for_user(&self.db, connection_id, &user_id)
            .await?
            .ok_or(CalendarConnectError::NotFound)?;

        let stats =
            calendar::sync_connection(&self.db, &self.http, &self.config, &connection).await?;

        let updated = calendar_connections::get_by_id(&self.db, connection_id)
            .await?
            .ok_or(CalendarConnectError::NotFound)?;

        Ok((stats, updated.last_synced_at.unwrap_or(crate::db::now_unix())))
    }

    pub async fn finish_oauth_callback(
        &self,
        code: &str,
        state: &str,
    ) -> Result<CalendarConnectionRow, CalendarConnectError> {
        match oauth::finish_callback(&self.db, &self.http, &self.config, code, state).await {
            Ok(connection) => {
                if let Err(error) =
                    calendar::sync_connection(&self.db, &self.http, &self.config, &connection)
                        .await
                {
                    tracing::warn!(
                        connection_id = %connection.id,
                        %error,
                        "initial calendar sync failed after OAuth"
                    );
                }
                Ok(connection)
            }
            Err(oauth::GoogleAuthError::AccountLinkedToOtherUser { email }) => {
                let hint = calendar_connections::find_owner_by_provider_email(
                    &self.db,
                    "google",
                    &email,
                )
                .await?
                .map(|owner| owner.owner_name)
                .unwrap_or_else(|| "another user".to_owned());

                Err(CalendarConnectError::AlreadyConnectedByOther {
                    email,
                    provider: "google".to_owned(),
                    other_account_hint: hint,
                })
            }
            Err(error) => Err(CalendarConnectError::from(error)),
        }
    }

    async fn require_user_id(&self, token: &str) -> Result<String, CalendarConnectError> {
        Ok(self.auth.get_session(token).await?.user.id)
    }
}
