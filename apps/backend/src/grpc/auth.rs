use tonic::{Request, Response, Status};

use crate::auth::{AuthError, AuthService as DomainAuthService};

pub mod proto {
    tonic::include_proto!("notetaker.v1");
}

use proto::{
    AuthError as ProtoAuthError, AuthResponse, AuthSuccess, GetSessionRequest, LoginRequest,
    LogoutRequest, LogoutResponse, SignUpRequest, User, auth_service_server::AuthService,
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
            Ok(session) => {
                tracing::info!(
                    user_id = %session.user.id,
                    username = %session.user.username,
                    "gRPC SignUp succeeded"
                );
                Ok(Response::new(success_response(session)))
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
                Ok(Response::new(success_response(session)))
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
                Ok(Response::new(success_response(session)))
            }
            Err(error) => {
                tracing::debug!(code = error.code(), "gRPC GetSession failed");
                Ok(Response::new(error_response(&error)))
            }
        }
    }
}

fn success_response(session: crate::auth::service::AuthSession) -> AuthResponse {
    AuthResponse {
        result: Some(proto::auth_response::Result::Success(AuthSuccess {
            user: Some(to_proto_user(&session.user)),
            token: session.token,
            expires_at: session.expires_at,
        })),
    }
}

fn error_response(error: &AuthError) -> AuthResponse {
    AuthResponse {
        result: Some(proto::auth_response::Result::Error(ProtoAuthError {
            code: error.code().to_owned(),
            message: error.to_string(),
        })),
    }
}

fn to_proto_user(user: &crate::db::users::UserRow) -> User {
    User {
        id: user.id.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
        username: user.username.clone(),
    }
}
