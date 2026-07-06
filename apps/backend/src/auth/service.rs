use std::sync::Arc;

use crate::db::DbPool;
use uuid::Uuid;

use crate::{
    auth::{AuthError, password},
    config::AuthConfig,
    db::{
        auth_email_tokens::{self, PURPOSE_RESET_PASSWORD, PURPOSE_VERIFY_EMAIL},
        users::{self, UserRow},
    },
    email::EmailSender,
};

#[derive(Clone)]
pub struct AuthService {
    db: DbPool,
    config: Arc<AuthConfig>,
    email: Option<EmailSender>,
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub user: UserRow,
    pub token: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct SignUpOutcome {
    pub user: UserRow,
    pub session: Option<AuthSession>,
}

impl AuthService {
    pub fn new(db: DbPool, config: Arc<AuthConfig>, email: Option<EmailSender>) -> Self {
        Self { db, config, email }
    }

    pub async fn sign_up(
        &self,
        name: &str,
        email: &str,
        username: &str,
        password: &str,
    ) -> Result<SignUpOutcome, AuthError> {
        validate_sign_up(name, email, username, password)?;

        let username = normalize_username(username);
        let email = normalize_email(email);
        let name = name.trim().to_owned();

        if users::get_user_by_username(&self.db, &username)
            .await?
            .is_some()
        {
            return Err(AuthError::UsernameTaken);
        }
        if users::get_user_by_email(&self.db, &email).await?.is_some() {
            return Err(AuthError::EmailTaken);
        }

        let password_hash = password::hash_password(password)?;
        let user_id = Uuid::new_v4().to_string();
        let verified_at = if self.config.require_email_verification {
            None
        } else {
            Some(crate::db::now_unix())
        };
        let user = users::create_user(
            &self.db,
            &user_id,
            &username,
            &email,
            &name,
            &password_hash,
            verified_at,
        )
        .await
        .map_err(map_unique_violation)?;

        if self.config.require_email_verification {
            self.issue_verification_email(&user).await?;
            return Ok(SignUpOutcome {
                user,
                session: None,
            });
        }

        let session = self.create_session_for_user(&user).await?;
        Ok(SignUpOutcome {
            user,
            session: Some(session),
        })
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<AuthSession, AuthError> {
        let username = username.trim();
        if username.is_empty() || password.is_empty() {
            return Err(AuthError::InvalidInput(
                "username and password are required".to_owned(),
            ));
        }

        let user = users::get_user_by_username(&self.db, username)
            .await?
            .ok_or(AuthError::InvalidCredentials)?;

        if !password::verify_password(password, &user.password_hash)? {
            return Err(AuthError::InvalidCredentials);
        }

        if self.config.require_email_verification && !user.is_email_verified() {
            return Err(AuthError::EmailNotVerified);
        }

        self.create_session_for_user(&user).await
    }

    pub async fn logout(&self, token: &str) -> Result<bool, AuthError> {
        if token.trim().is_empty() {
            return Ok(false);
        }
        let deleted = users::delete_session(&self.db, token).await?;
        Ok(deleted)
    }

    pub async fn get_session(&self, token: &str) -> Result<AuthSession, AuthError> {
        if token.trim().is_empty() {
            return Err(AuthError::SessionNotFound);
        }

        let session = users::get_session(&self.db, token)
            .await?
            .ok_or(AuthError::SessionNotFound)?;

        let user = users::get_user_by_id(&self.db, &session.user_id)
            .await?
            .ok_or(AuthError::SessionNotFound)?;

        users::touch_session(&self.db, token).await?;

        Ok(AuthSession {
            user,
            token: session.token,
            expires_at: session.expires_at,
        })
    }

    pub async fn require_user_id(&self, token: &str) -> Result<String, AuthError> {
        Ok(self.get_session(token).await?.user.id)
    }

    pub async fn request_password_reset(&self, email: &str) -> Result<String, AuthError> {
        let email = normalize_email(email);
        if email.is_empty() || !email.contains('@') {
            return Err(AuthError::InvalidInput("invalid email address".to_owned()));
        }

        if let Some(user) = users::get_user_by_email(&self.db, &email).await? {
            self.issue_password_reset_email(&user).await?;
        }

        Ok(
            "If an account exists for that email, a password reset link has been sent."
                .to_owned(),
        )
    }

    pub async fn reset_password(
        &self,
        token: &str,
        new_password: &str,
    ) -> Result<String, AuthError> {
        if token.trim().is_empty() {
            return Err(AuthError::InvalidToken);
        }
        if new_password.len() < 8 {
            return Err(AuthError::InvalidInput(
                "password must be at least 8 characters".to_owned(),
            ));
        }

        let row = auth_email_tokens::get_valid_token(
            &self.db,
            token.trim(),
            PURPOSE_RESET_PASSWORD,
        )
        .await?
        .ok_or(AuthError::InvalidToken)?;

        let password_hash = password::hash_password(new_password)?;
        users::update_password_hash(&self.db, &row.user_id, &password_hash).await?;
        auth_email_tokens::delete_token(&self.db, &row.token).await?;
        auth_email_tokens::delete_tokens_for_user_purpose(
            &self.db,
            &row.user_id,
            PURPOSE_RESET_PASSWORD,
        )
        .await?;

        Ok("Your password has been updated. You can sign in now.".to_owned())
    }

    pub async fn verify_email(&self, token: &str) -> Result<AuthSession, AuthError> {
        if token.trim().is_empty() {
            return Err(AuthError::InvalidToken);
        }

        let row = auth_email_tokens::get_valid_token(
            &self.db,
            token.trim(),
            PURPOSE_VERIFY_EMAIL,
        )
        .await?
        .ok_or(AuthError::InvalidToken)?;

        let user = users::get_user_by_id(&self.db, &row.user_id)
            .await?
            .ok_or(AuthError::InvalidToken)?;

        if user.is_email_verified() {
            auth_email_tokens::delete_token(&self.db, &row.token).await?;
            return self.create_session_for_user(&user).await;
        }

        users::mark_email_verified(&self.db, &row.user_id).await?;
        auth_email_tokens::delete_token(&self.db, &row.token).await?;
        auth_email_tokens::delete_tokens_for_user_purpose(
            &self.db,
            &row.user_id,
            PURPOSE_VERIFY_EMAIL,
        )
        .await?;

        let user = users::get_user_by_id(&self.db, &row.user_id)
            .await?
            .ok_or(AuthError::InvalidToken)?;

        self.create_session_for_user(&user).await
    }

    pub async fn resend_verification_email(&self, email: &str) -> Result<String, AuthError> {
        let email = normalize_email(email);
        if email.is_empty() || !email.contains('@') {
            return Err(AuthError::InvalidInput("invalid email address".to_owned()));
        }

        if let Some(user) = users::get_user_by_email(&self.db, &email).await? {
            if user.is_email_verified() {
                return Err(AuthError::EmailAlreadyVerified);
            }
            self.issue_verification_email(&user).await?;
        }

        Ok("If an account exists and is not yet verified, a verification email has been sent."
            .to_owned())
    }

    async fn create_session_for_user(&self, user: &UserRow) -> Result<AuthSession, AuthError> {
        let token = Uuid::new_v4().to_string();
        let expires_at = crate::db::now_unix() + self.config.session_ttl_secs as i64;
        let session = users::create_session(&self.db, &token, &user.id, expires_at).await?;

        Ok(AuthSession {
            user: user.clone(),
            token: session.token,
            expires_at: session.expires_at,
        })
    }

    async fn issue_verification_email(&self, user: &UserRow) -> Result<(), AuthError> {
        let token = Uuid::new_v4().to_string();
        let expires_at =
            crate::db::now_unix() + self.config.email_verification_token_ttl_secs as i64;
        auth_email_tokens::delete_tokens_for_user_purpose(
            &self.db,
            &user.id,
            PURPOSE_VERIFY_EMAIL,
        )
        .await?;
        auth_email_tokens::create_token(
            &self.db,
            &token,
            &user.id,
            PURPOSE_VERIFY_EMAIL,
            expires_at,
        )
        .await?;

        let verify_url = format!(
            "{}/verify-email?token={}",
            self.config.app_base_url.trim_end_matches('/'),
            token
        );

        if let Some(email) = &self.email {
            email
                .send_verification_email(&user.email, &user.name, &verify_url)
                .await
                .map_err(|error| AuthError::EmailSendFailed(error.to_string()))?;
        } else {
            tracing::info!(
                user_id = %user.id,
                email = %user.email,
                %verify_url,
                "email sender not configured; verification link logged for development"
            );
        }

        Ok(())
    }

    async fn issue_password_reset_email(&self, user: &UserRow) -> Result<(), AuthError> {
        let token = Uuid::new_v4().to_string();
        let expires_at = crate::db::now_unix() + self.config.password_reset_token_ttl_secs as i64;
        auth_email_tokens::delete_tokens_for_user_purpose(
            &self.db,
            &user.id,
            PURPOSE_RESET_PASSWORD,
        )
        .await?;
        auth_email_tokens::create_token(
            &self.db,
            &token,
            &user.id,
            PURPOSE_RESET_PASSWORD,
            expires_at,
        )
        .await?;

        let reset_url = format!(
            "{}/reset-password?token={}",
            self.config.app_base_url.trim_end_matches('/'),
            token
        );

        if let Some(email) = &self.email {
            email
                .send_password_reset_email(&user.email, &user.name, &reset_url)
                .await
                .map_err(|error| AuthError::EmailSendFailed(error.to_string()))?;
        } else {
            tracing::info!(
                user_id = %user.id,
                email = %user.email,
                %reset_url,
                "email sender not configured; password reset link logged for development"
            );
        }

        Ok(())
    }
}

fn validate_sign_up(
    name: &str,
    email: &str,
    username: &str,
    password: &str,
) -> Result<(), AuthError> {
    if name.trim().is_empty()
        || email.trim().is_empty()
        || username.trim().is_empty()
        || password.is_empty()
    {
        return Err(AuthError::InvalidInput(
            "all fields are required".to_owned(),
        ));
    }
    if password.len() < 8 {
        return Err(AuthError::InvalidInput(
            "password must be at least 8 characters".to_owned(),
        ));
    }
    if username.trim().len() < 3 {
        return Err(AuthError::InvalidInput(
            "username must be at least 3 characters".to_owned(),
        ));
    }
    if !email.contains('@') {
        return Err(AuthError::InvalidInput("invalid email address".to_owned()));
    }
    Ok(())
}

fn normalize_username(username: &str) -> String {
    username.trim().to_owned()
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn map_unique_violation(error: sqlx::Error) -> AuthError {
    if let sqlx::Error::Database(db_error) = &error {
        if db_error.is_unique_violation() {
            let message = db_error.message().to_ascii_lowercase();
            if message.contains("users.username") {
                return AuthError::UsernameTaken;
            }
            if message.contains("users.email") {
                return AuthError::EmailTaken;
            }
        }
    }
    AuthError::Internal(error.into())
}

#[cfg(test)]
mod tests {
    use super::validate_sign_up;

    #[test]
    fn rejects_short_password_on_sign_up() {
        let error = validate_sign_up("Alex", "alex@acme.co", "alex", "short").unwrap_err();
        assert!(matches!(error, crate::auth::AuthError::InvalidInput(_)));
    }
}
