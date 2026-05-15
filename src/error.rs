use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    GoogleAuth(#[from] crate::google::oauth::GoogleAuthError),
    #[error(transparent)]
    Calendar(#[from] crate::google::calendar::CalendarError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            AppError::GoogleAuth(crate::google::oauth::GoogleAuthError::MissingConfig)
            | AppError::GoogleAuth(crate::google::oauth::GoogleAuthError::NotConnected)
            | AppError::GoogleAuth(crate::google::oauth::GoogleAuthError::StateMismatch)
            | AppError::GoogleAuth(crate::google::oauth::GoogleAuthError::MissingRefreshToken) => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        tracing::error!(error = %self, "request failed");
        (status, self.to_string()).into_response()
    }
}
