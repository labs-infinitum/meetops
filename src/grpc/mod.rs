use std::sync::Arc;

use reqwest::Client;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tower_http::{
    cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

use crate::{
    auth::AuthService,
    calendar::CalendarService,
    config::{Config, GrpcConfig},
    meeting::MeetingService,
    recording::RecordingService,
    storage::RecordingsStore,
};

pub mod auth;
pub mod calendar;
pub mod meeting;
pub mod recording;

#[derive(Clone)]
pub struct GrpcState {
    pub config: Arc<Config>,
    pub db: SqlitePool,
    pub http: Client,
    pub auth: AuthService,
    pub recordings: Arc<RecordingsStore>,
    pub chrome_sessions: Arc<Semaphore>,
}

pub async fn serve(listener: TcpListener, grpc_config: Arc<GrpcConfig>, state: GrpcState) -> anyhow::Result<()> {
    let addr = listener.local_addr()?;
    let auth_service = auth::AuthGrpcService {
        inner: state.auth.clone(),
    };
    let calendar_service = calendar::CalendarGrpcService {
        inner: CalendarService::new(
            state.db.clone(),
            state.http.clone(),
            Arc::clone(&state.config),
            state.auth.clone(),
        ),
    };
    let meeting_service = meeting::MeetingGrpcService {
        inner: MeetingService::new(state.db.clone(), state.auth.clone()),
    };
    let recording_service = recording::RecordingGrpcService {
        inner: RecordingService::new(
            state.db.clone(),
            state.auth.clone(),
            Arc::clone(&state.recordings),
            Arc::clone(&state.config),
            Arc::clone(&state.chrome_sessions),
        ),
    };

    let cors = CorsLayer::new()
        .allow_origin(parse_allow_origin(&grpc_config.allow_origin)?)
        .allow_methods(AllowMethods::list([
            http::Method::GET,
            http::Method::POST,
            http::Method::OPTIONS,
        ]))
        .allow_headers(AllowHeaders::any());

    let trace = TraceLayer::new_for_http()
        .make_span_with(|request: &http::Request<_>| {
            tracing::info_span!(
                "grpc_request",
                method = %request.method(),
                uri = %request.uri(),
            )
        })
        .on_request(|request: &http::Request<_>, _span: &tracing::Span| {
            tracing::debug!(method = %request.method(), uri = %request.uri(), "gRPC request started");
        })
        .on_response(
            |response: &http::Response<_>, latency: std::time::Duration, _span: &tracing::Span| {
                tracing::info!(
                    status = %response.status(),
                    latency_ms = latency.as_millis(),
                    "gRPC request completed"
                );
            },
        )
        .on_failure(
            |error: tower_http::classify::ServerErrorsFailureClass,
             latency: std::time::Duration,
             _span: &tracing::Span| {
                tracing::warn!(
                    ?error,
                    latency_ms = latency.as_millis(),
                    "gRPC request failed"
                );
            },
        );

    tracing::info!(%addr, "gRPC server listening (grpc-web enabled)");

    Server::builder()
        .accept_http1(true)
        .layer(trace)
        .layer(cors)
        .add_service(tonic_web::enable(
            auth::proto::auth_service_server::AuthServiceServer::new(auth_service),
        ))
        .add_service(tonic_web::enable(
            calendar::proto::calendar_service_server::CalendarServiceServer::new(calendar_service),
        ))
        .add_service(tonic_web::enable(
            meeting::proto::meeting_service_server::MeetingServiceServer::new(meeting_service),
        ))
        .add_service(tonic_web::enable(
            recording::proto::recording_service_server::RecordingServiceServer::new(recording_service),
        ))
        .serve_with_incoming(TcpListenerStream::new(listener))
        .await?;

    Ok(())
}

fn parse_allow_origin(origin: &str) -> anyhow::Result<AllowOrigin> {
    if origin == "*" {
        return Ok(AllowOrigin::any());
    }
    Ok(AllowOrigin::list([http::HeaderValue::from_str(origin)?]))
}
