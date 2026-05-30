use std::{sync::Arc, time::Duration};

use http::{HeaderValue, Method};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tower_http::{
    cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

use crate::{auth::AuthService, config::GrpcConfig};

pub mod auth;

pub async fn serve(
    listener: TcpListener,
    config: Arc<GrpcConfig>,
    auth: AuthService,
) -> anyhow::Result<()> {
    let addr = listener.local_addr()?;
    let auth_service = auth::AuthGrpcService { inner: auth };

    let cors = CorsLayer::new()
        .allow_origin(parse_allow_origin(&config.allow_origin)?)
        .allow_methods(AllowMethods::list([
            Method::GET,
            Method::POST,
            Method::OPTIONS,
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
            |response: &http::Response<_>, latency: Duration, _span: &tracing::Span| {
                tracing::info!(
                    status = %response.status(),
                    latency_ms = latency.as_millis(),
                    "gRPC request completed"
                );
            },
        )
        .on_failure(
            |error: tower_http::classify::ServerErrorsFailureClass,
             latency: Duration,
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
        .serve_with_incoming(TcpListenerStream::new(listener))
        .await?;

    Ok(())
}

fn parse_allow_origin(origin: &str) -> anyhow::Result<AllowOrigin> {
    if origin == "*" {
        return Ok(AllowOrigin::any());
    }
    Ok(AllowOrigin::list([HeaderValue::from_str(origin)?]))
}
