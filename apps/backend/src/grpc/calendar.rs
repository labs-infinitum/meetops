use tonic::{Request, Response, Status};

use crate::calendar::{CalendarConnectError, CalendarService as DomainCalendarService};

pub mod proto {
    tonic::include_proto!("notetaker.v1");
}

use proto::{
    CalendarConnection, CalendarError, DisconnectRequest, DisconnectResponse, DisconnectSuccess,
    ListConnectionsRequest, ListConnectionsResponse, ListConnectionsSuccess, StartOAuthRequest,
    StartOAuthResponse, StartOAuthSuccess, SyncNowRequest, SyncNowResponse, SyncNowSuccess,
    calendar_service_server::CalendarService,
};

pub struct CalendarGrpcService {
    pub inner: DomainCalendarService,
}

#[tonic::async_trait]
impl CalendarService for CalendarGrpcService {
    async fn list_connections(
        &self,
        request: Request<ListConnectionsRequest>,
    ) -> Result<Response<ListConnectionsResponse>, Status> {
        let req = request.into_inner();
        match self.inner.list_connections(&req.token).await {
            Ok(connections) => Ok(Response::new(ListConnectionsResponse {
                result: Some(proto::list_connections_response::Result::Success(
                    ListConnectionsSuccess {
                        connections: connections.into_iter().map(to_proto_connection).collect(),
                    },
                )),
            })),
            Err(error) => {
                tracing::warn!(code = error.code(), "gRPC ListConnections failed");
                Ok(Response::new(ListConnectionsResponse {
                    result: Some(proto::list_connections_response::Result::Error(
                        to_proto_error(&error),
                    )),
                }))
            }
        }
    }

    async fn start_o_auth(
        &self,
        request: Request<StartOAuthRequest>,
    ) -> Result<Response<StartOAuthResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(provider = %req.provider, "gRPC StartOAuth");
        let label = if req.label.is_empty() {
            None
        } else {
            Some(req.label.as_str())
        };
        let color = if req.color.is_empty() {
            None
        } else {
            Some(req.color.as_str())
        };

        match self
            .inner
            .start_oauth(&req.token, &req.provider, label, color)
            .await
        {
            Ok(redirect_url) => Ok(Response::new(StartOAuthResponse {
                result: Some(proto::start_o_auth_response::Result::Success(
                    StartOAuthSuccess { redirect_url },
                )),
            })),
            Err(error) => {
                tracing::warn!(code = error.code(), "gRPC StartOAuth failed");
                Ok(Response::new(StartOAuthResponse {
                    result: Some(proto::start_o_auth_response::Result::Error(to_proto_error(
                        &error,
                    ))),
                }))
            }
        }
    }

    async fn disconnect(
        &self,
        request: Request<DisconnectRequest>,
    ) -> Result<Response<DisconnectResponse>, Status> {
        let req = request.into_inner();
        match self.inner.disconnect(&req.token, &req.connection_id).await {
            Ok(ok) => Ok(Response::new(DisconnectResponse {
                result: Some(proto::disconnect_response::Result::Success(
                    DisconnectSuccess { ok },
                )),
            })),
            Err(error) => Ok(Response::new(DisconnectResponse {
                result: Some(proto::disconnect_response::Result::Error(to_proto_error(
                    &error,
                ))),
            })),
        }
    }

    async fn sync_now(
        &self,
        request: Request<SyncNowRequest>,
    ) -> Result<Response<SyncNowResponse>, Status> {
        let req = request.into_inner();
        match self.inner.sync_now(&req.token, &req.connection_id).await {
            Ok((stats, last_synced_at)) => Ok(Response::new(SyncNowResponse {
                result: Some(proto::sync_now_response::Result::Success(SyncNowSuccess {
                    seen: stats.seen as i32,
                    upserted: stats.upserted as i32,
                    deleted: stats.deleted as i32,
                    last_synced_at,
                })),
            })),
            Err(error) => Ok(Response::new(SyncNowResponse {
                result: Some(proto::sync_now_response::Result::Error(to_proto_error(
                    &error,
                ))),
            })),
        }
    }
}

fn to_proto_connection(
    row: crate::db::calendar_connections::CalendarConnectionListItem,
) -> CalendarConnection {
    CalendarConnection {
        id: row.id,
        provider: row.provider,
        email: row.email,
        label: row.label,
        color: row.color,
        connected: true,
        last_synced_at: row.last_synced_at.unwrap_or(0),
        meeting_count: row.meeting_count as i32,
    }
}

fn to_proto_error(error: &CalendarConnectError) -> CalendarError {
    CalendarError {
        code: error.code().to_owned(),
        message: error.to_string(),
        email: error.email().unwrap_or("").to_owned(),
        provider: error.provider().unwrap_or("").to_owned(),
        other_account_hint: error.other_account_hint().unwrap_or("").to_owned(),
    }
}
