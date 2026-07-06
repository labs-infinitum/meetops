use reqwest::Client;
use serde::Serialize;
use thiserror::Error;

use crate::config::EmailConfig;

#[derive(Debug, Error)]
pub enum EmailError {
    #[error("failed to send email: {0}")]
    SendFailed(String),
}

#[derive(Clone)]
pub struct EmailSender {
    http: Client,
    api_key: String,
    from: String,
}

#[derive(Serialize)]
struct ResendEmailRequest<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: &'a str,
    html: &'a str,
}

impl EmailSender {
    pub fn from_config(http: Client, config: &EmailConfig) -> Option<Self> {
        let api_key = std::env::var(&config.api_key_env)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())?;
        let from_email = config.from_email.trim();
        if from_email.is_empty() {
            tracing::warn!(
                env = %config.api_key_env,
                "Resend API key is set but email.from_email is empty; transactional email disabled"
            );
            return None;
        }

        let from = if config.from_name.trim().is_empty() {
            from_email.to_owned()
        } else {
            format!("{} <{}>", config.from_name.trim(), from_email)
        };

        Some(Self {
            http,
            api_key,
            from,
        })
    }

    pub async fn send_verification_email(
        &self,
        to: &str,
        name: &str,
        verify_url: &str,
    ) -> Result<(), EmailError> {
        let greeting = if name.trim().is_empty() {
            "Hi".to_owned()
        } else {
            format!("Hi {}", name.trim())
        };
        let subject = "Verify your email address";
        let html = format!(
            r#"<p>{greeting},</p>
<p>Thanks for signing up. Please verify your email address by clicking the link below:</p>
<p><a href="{verify_url}">Verify email</a></p>
<p>Or copy this URL into your browser:</p>
<p>{verify_url}</p>
<p>If you did not create an account, you can ignore this email.</p>"#
        );
        self.send(to, subject, &html).await
    }

    pub async fn send_password_reset_email(
        &self,
        to: &str,
        name: &str,
        reset_url: &str,
    ) -> Result<(), EmailError> {
        let greeting = if name.trim().is_empty() {
            "Hi".to_owned()
        } else {
            format!("Hi {}", name.trim())
        };
        let subject = "Reset your password";
        let html = format!(
            r#"<p>{greeting},</p>
<p>We received a request to reset your password. Click the link below to choose a new password:</p>
<p><a href="{reset_url}">Reset password</a></p>
<p>Or copy this URL into your browser:</p>
<p>{reset_url}</p>
<p>If you did not request a password reset, you can ignore this email.</p>"#
        );
        self.send(to, subject, &html).await
    }

    async fn send(&self, to: &str, subject: &str, html: &str) -> Result<(), EmailError> {
        let payload = ResendEmailRequest {
            from: &self.from,
            to: [to],
            subject,
            html,
        };

        let response = self
            .http
            .post("https://api.resend.com/emails")
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|error| EmailError::SendFailed(error.to_string()))?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_owned());
        Err(EmailError::SendFailed(format!("HTTP {status}: {body}")))
    }
}
