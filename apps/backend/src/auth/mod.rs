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
    #[error("email address is not verified")]
    EmailNotVerified,
    #[error("email is already verified")]
    EmailAlreadyVerified,
    #[error("invalid or expired token")]
    InvalidToken,
    #[error("failed to send email: {0}")]
    EmailSendFailed(String),
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
            Self::EmailNotVerified => "email_not_verified",
            Self::EmailAlreadyVerified => "email_already_verified",
            Self::InvalidToken => "invalid_token",
            Self::EmailSendFailed(_) => "email_send_failed",
            Self::Internal(_) => "internal_error",
        }
    }
}

pub mod password;
pub mod service;

pub use service::AuthService;
