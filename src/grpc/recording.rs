use tonic::{Request, Response, Status};

use crate::recording::{duration_secs, RecordingService as DomainRecordingService, RecordingServiceError};

pub mod proto {
    tonic::include_proto!("notetaker.v1");
}

use proto::{
    BotRun, GetBotRunRequest, GetBotRunResponse, GetPlaybackUrlRequest, GetPlaybackUrlResponse,
    ListBotRunsRequest, ListBotRunsResponse, ListBotRunsSuccess, PlaybackUrlSuccess,
    RecordingError, recording_service_server::RecordingService,
};

pub struct RecordingGrpcService {
    pub inner: DomainRecordingService,
}

#[tonic::async_trait]
impl RecordingService for RecordingGrpcService {
    async fn list_bot_runs(
        &self,
        request: Request<ListBotRunsRequest>,
    ) -> Result<Response<ListBotRunsResponse>, Status> {
        let req = request.into_inner();
        let limit = if req.limit == 0 { 50 } else { req.limit };
        match self.inner.list_bot_runs(&req.token, limit).await {
            Ok(runs) => Ok(Response::new(ListBotRunsResponse {
                result: Some(proto::list_bot_runs_response::Result::Success(
                    ListBotRunsSuccess {
                        runs: runs.iter().map(to_proto_run).collect(),
                    },
                )),
            })),
            Err(error) => Ok(Response::new(ListBotRunsResponse {
                result: Some(proto::list_bot_runs_response::Result::Error(to_proto_error(
                    &error,
                ))),
            })),
        }
    }

    async fn get_bot_run(
        &self,
        request: Request<GetBotRunRequest>,
    ) -> Result<Response<GetBotRunResponse>, Status> {
        let req = request.into_inner();
        match self.inner.get_bot_run(&req.token, req.run_id).await {
            Ok(run) => Ok(Response::new(GetBotRunResponse {
                result: Some(proto::get_bot_run_response::Result::Success(to_proto_run(&run))),
            })),
            Err(error) => Ok(Response::new(GetBotRunResponse {
                result: Some(proto::get_bot_run_response::Result::Error(to_proto_error(
                    &error,
                ))),
            })),
        }
    }

    async fn get_playback_url(
        &self,
        request: Request<GetPlaybackUrlRequest>,
    ) -> Result<Response<GetPlaybackUrlResponse>, Status> {
        let req = request.into_inner();
        match self.inner.playback_url(&req.token, req.run_id).await {
            Ok((url, expires_at)) => Ok(Response::new(GetPlaybackUrlResponse {
                result: Some(proto::get_playback_url_response::Result::Success(
                    PlaybackUrlSuccess { url, expires_at },
                )),
            })),
            Err(error) => Ok(Response::new(GetPlaybackUrlResponse {
                result: Some(proto::get_playback_url_response::Result::Error(to_proto_error(
                    &error,
                ))),
            })),
        }
    }
}

fn to_proto_run(run: &crate::db::models::BotRunListItem) -> BotRun {
    BotRun {
        id: run.id,
        meeting_id: run.meeting_id.clone(),
        meeting_title: run.title.clone(),
        status: run.status.clone(),
        error: run.error.clone().unwrap_or_default(),
        meeting_start_at: run.meeting_start_at,
        started_at: run.started_at.unwrap_or(0),
        ended_at: run.ended_at.unwrap_or(0),
        created_at: run.created_at,
        recording_path: run.recording_path.clone().unwrap_or_default(),
        recording_storage: run.recording_storage.clone(),
        duration_secs: duration_secs(run),
    }
}

fn to_proto_error(error: &RecordingServiceError) -> RecordingError {
    RecordingError {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}
