use tonic::{Request, Response, Status};

use crate::auth::{AuthError, AuthService as DomainAuthService};

pub mod proto {
    tonic::include_proto!("notetaker.v1");
}

use proto::{
    AuthError as ProtoAuthError, AuthMessageResponse, AuthMessageSuccess, AuthResponse,
    AuthSuccess, GetSessionRequest, LoginRequest, LogoutRequest, LogoutResponse,
    RequestPasswordResetRequest, ResendVerificationEmailRequest, ResetPasswordRequest,
    SignUpRequest, User, VerifyEmailRequest, auth_service_server::AuthService,
};

pub struct AuthGrpcService {
    pub inner: DomainAuthService,
}

#[tonic::async_trait]
impl AuthService for AuthGrpcService {
    async fn sign_up(
        &self,
        request: Request<SignUpRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(username = %req.username, email = %req.email, "gRPC SignUp");
        match self
            .inner
            .sign_up(&req.name, &req.email, &req.username, &req.password)
            .await
        {
            Ok(outcome) => {
                tracing::info!(
                    user_id = %outcome.user.id,
                    username = %outcome.user.username,
                    verification_required = outcome.session.is_none(),
                    "gRPC SignUp succeeded"
                );
                Ok(Response::new(sign_up_response(outcome)))
            }
            Err(error) => {
                tracing::warn!(code = error.code(), username = %req.username, "gRPC SignUp failed");
                Ok(Response::new(error_response(&error)))
            }
        }
    }

    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(username = %req.username, "gRPC Login");
        match self.inner.login(&req.username, &req.password).await {
            Ok(session) => {
                tracing::info!(
                    user_id = %session.user.id,
                    username = %session.user.username,
                    "gRPC Login succeeded"
                );
                Ok(Response::new(session_response(session)))
            }
            Err(error) => {
                tracing::warn!(code = error.code(), username = %req.username, "gRPC Login failed");
                Ok(Response::new(error_response(&error)))
            }
        }
    }

    async fn logout(
        &self,
        request: Request<LogoutRequest>,
    ) -> Result<Response<LogoutResponse>, Status> {
        let req = request.into_inner();
        tracing::debug!("gRPC Logout");
        let ok = self
            .inner
            .logout(&req.token)
            .await
            .map_err(|error| Status::internal(error.to_string()))?;
        tracing::info!(ok, "gRPC Logout completed");
        Ok(Response::new(LogoutResponse { ok }))
    }

    async fn get_session(
        &self,
        request: Request<GetSessionRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        tracing::debug!("gRPC GetSession");
        let req = request.into_inner();
        match self.inner.get_session(&req.token).await {
            Ok(session) => {
                tracing::debug!(
                    user_id = %session.user.id,
                    username = %session.user.username,
                    "gRPC GetSession succeeded"
                );
                Ok(Response::new(session_response(session)))
            }
            Err(error) => {
                tracing::debug!(code = error.code(), "gRPC GetSession failed");
                Ok(Response::new(error_response(&error)))
            }
        }
    }

    async fn request_password_reset(
        &self,
        request: Request<RequestPasswordResetRequest>,
    ) -> Result<Response<AuthMessageResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(email = %req.email, "gRPC RequestPasswordReset");
        match self.inner.request_password_reset(&req.email).await {
            Ok(message) => Ok(Response::new(message_success_response(message))),
            Err(error) => Ok(Response::new(message_error_response(&error))),
        }
    }

    async fn reset_password(
        &self,
        request: Request<ResetPasswordRequest>,
    ) -> Result<Response<AuthMessageResponse>, Status> {
        let req = request.into_inner();
        tracing::info!("gRPC ResetPassword");
        match self
            .inner
            .reset_password(&req.token, &req.new_password)
            .await
        {
            Ok(message) => Ok(Response::new(message_success_response(message))),
            Err(error) => Ok(Response::new(message_error_response(&error))),
        }
    }

    async fn verify_email(
        &self,
        request: Request<VerifyEmailRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();
        tracing::info!("gRPC VerifyEmail");
        match self.inner.verify_email(&req.token).await {
            Ok(session) => Ok(Response::new(session_response(session))),
            Err(error) => Ok(Response::new(error_response(&error))),
        }
    }

    async fn resend_verification_email(
        &self,
        request: Request<ResendVerificationEmailRequest>,
    ) -> Result<Response<AuthMessageResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(email = %req.email, "gRPC ResendVerificationEmail");
        match self.inner.resend_verification_email(&req.email).await {
            Ok(message) => Ok(Response::new(message_success_response(message))),
            Err(error) => Ok(Response::new(message_error_response(&error))),
        }
    }
}

fn sign_up_response(outcome: crate::auth::service::SignUpOutcome) -> AuthResponse {
    if let Some(session) = outcome.session {
        return session_response(session);
    }

    AuthResponse {
        result: Some(proto::auth_response::Result::Success(AuthSuccess {
            user: Some(to_proto_user(&outcome.user)),
            token: String::new(),
            expires_at: 0,
            email_verification_required: true,
        })),
    }
}

fn session_response(session: crate::auth::service::AuthSession) -> AuthResponse {
    AuthResponse {
        result: Some(proto::auth_response::Result::Success(AuthSuccess {
            user: Some(to_proto_user(&session.user)),
            token: session.token,
            expires_at: session.expires_at,
            email_verification_required: false,
        })),
    }
}

fn error_response(error: &AuthError) -> AuthResponse {
    AuthResponse {
        result: Some(proto::auth_response::Result::Error(to_proto_error(error))),
    }
}

/// Internal errors are logged server-side; clients get a generic message.
fn to_proto_error(error: &AuthError) -> ProtoAuthError {
    let message = match error {
        AuthError::Internal(detail) => {
            tracing::error!(%detail, "internal auth service error");
            "internal server error".to_owned()
        }
        AuthError::EmailSendFailed(detail) => {
            tracing::error!(%detail, "failed to send auth email");
            "failed to send email; try again later".to_owned()
        }
        other => other.to_string(),
    };
    ProtoAuthError {
        code: error.code().to_owned(),
        message,
    }
}

fn message_success_response(message: String) -> AuthMessageResponse {
    AuthMessageResponse {
        result: Some(proto::auth_message_response::Result::Success(
            AuthMessageSuccess { message },
        )),
    }
}

fn message_error_response(error: &AuthError) -> AuthMessageResponse {
    AuthMessageResponse {
        result: Some(proto::auth_message_response::Result::Error(to_proto_error(
            error,
        ))),
    }
}

fn to_proto_user(user: &crate::db::users::UserRow) -> User {
    User {
        id: user.id.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
        username: user.username.clone(),
        email_verified: user.is_email_verified(),
    }
}
