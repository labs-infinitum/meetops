use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("username already taken")]
    UsernameTaken,
    #[error("email already registered")]
    EmailTaken,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("session not found or expired")]
    SessionNotFound,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<sqlx::Error> for AuthError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl AuthError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "invalid_input",
            Self::UsernameTaken => "username_taken",
            Self::EmailTaken => "email_taken",
            Self::InvalidCredentials => "invalid_credentials",
            Self::SessionNotFound => "session_not_found",
            Self::Internal(_) => "internal_error",
        }
    }
}

pub mod password;
pub mod service;

pub use service::AuthService;
