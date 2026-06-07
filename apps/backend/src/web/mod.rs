//! Minimal HTTP routes (no UI): health check and Google OAuth callback for the React app.

use std::sync::Arc;

use crate::db::DbPool;
use axum::{
    Router,
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    routing::get,
};
use reqwest::Client;
use serde::Deserialize;
use tower_http::trace::TraceLayer;

use crate::{
    auth::AuthService, calendar::CalendarService, config::Config, error::AppError,
    google::oauth::GoogleAuthError,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: DbPool,
    pub http: Client,
    pub auth: AuthService,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/auth/google/callback", get(auth_google_callback))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

#[derive(Debug, Deserialize)]
struct GoogleCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn auth_google_callback(
    State(state): State<AppState>,
    Query(query): Query<GoogleCallbackQuery>,
) -> Result<impl IntoResponse, AppError> {
    if query.error.is_some() {
        return Ok(Redirect::temporary(&oauth_redirect_url(
            &state.config,
            "oauth_denied",
            None,
            None,
            None,
        ))
        .into_response());
    }

    let code = query
        .code
        .ok_or(GoogleAuthError::Token("missing code".to_owned()))?;
    let state_value = query.state.ok_or(GoogleAuthError::StateMismatch)?;

    let calendar = CalendarService::new(
        state.db.clone(),
        state.http.clone(),
        Arc::clone(&state.config),
        state.auth.clone(),
    );

    match calendar.finish_oauth_callback(&code, &state_value).await {
        Ok(connection) => Ok(Redirect::temporary(&oauth_redirect_url(
            &state.config,
            "connected",
            Some("google"),
            Some(&connection.email),
            None,
        ))
        .into_response()),
        Err(error) => {
            if matches!(
                error,
                crate::calendar::CalendarConnectError::AlreadyConnectedByOther { .. }
            ) {
                Ok(Redirect::temporary(&oauth_redirect_url(
                    &state.config,
                    error.code(),
                    error.provider(),
                    error.email(),
                    error.other_account_hint(),
                ))
                .into_response())
            } else if matches!(error, crate::calendar::CalendarConnectError::GoogleAuth(_)) {
                Ok(Redirect::temporary(&oauth_redirect_url(
                    &state.config,
                    "oauth_failed",
                    Some("google"),
                    None,
                    None,
                ))
                .into_response())
            } else {
                Err(error.into())
            }
        }
    }
}

fn oauth_redirect_url(
    config: &Config,
    code: &str,
    provider: Option<&str>,
    email: Option<&str>,
    other_hint: Option<&str>,
) -> String {
    let base = config.google.frontend_redirect_url.trim_end_matches('/');
    let mut url = format!("{base}?oauth={}", urlencoding::encode(code));
    if let Some(provider) = provider {
        url.push_str(&format!("&provider={}", urlencoding::encode(provider)));
    }
    if let Some(email) = email.filter(|value| !value.is_empty()) {
        url.push_str(&format!("&email={}", urlencoding::encode(email)));
    }
    if let Some(hint) = other_hint.filter(|value| !value.is_empty()) {
        url.push_str(&format!("&other_hint={}", urlencoding::encode(hint)));
    }
    url
}
