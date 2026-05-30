use std::sync::Arc;

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    auth::{password, AuthError},
    config::AuthConfig,
    db::users::{self, UserRow},
};

#[derive(Clone)]
pub struct AuthService {
    db: SqlitePool,
    config: Arc<AuthConfig>,
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub user: UserRow,
    pub token: String,
    pub expires_at: i64,
}

impl AuthService {
    pub fn new(db: SqlitePool, config: Arc<AuthConfig>) -> Self {
        Self { db, config }
    }

    pub async fn sign_up(
        &self,
        name: &str,
        email: &str,
        username: &str,
        password: &str,
    ) -> Result<AuthSession, AuthError> {
        validate_sign_up(name, email, username, password)?;

        let username = normalize_username(username);
        let email = normalize_email(email);
        let name = name.trim().to_owned();

        if users::get_user_by_username(&self.db, &username).await?.is_some() {
            return Err(AuthError::UsernameTaken);
        }
        if users::get_user_by_email(&self.db, &email).await?.is_some() {
            return Err(AuthError::EmailTaken);
        }

        let password_hash = password::hash_password(password)?;
        let user_id = Uuid::new_v4().to_string();
        let user = users::create_user(
            &self.db,
            &user_id,
            &username,
            &email,
            &name,
            &password_hash,
        )
        .await
        .map_err(map_unique_violation)?;

        self.create_session_for_user(&user).await
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
        return Err(AuthError::InvalidInput("all fields are required".to_owned()));
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
