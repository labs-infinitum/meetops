use tonic::{Request, Response, Status};

use crate::meeting::{MeetingError, MeetingService as DomainMeetingService};

pub mod proto {
    tonic::include_proto!("notetaker.v1");
}

use proto::{
    DashboardSummary, GetDashboardSummaryRequest, GetDashboardSummaryResponse, GetMeetingRequest,
    GetMeetingResponse, ListMeetingsRequest, ListMeetingsResponse, ListMeetingsSuccess, Meeting,
    MeetingError as ProtoMeetingError, SetAutoRecordRequest, SetAutoRecordResponse,
    SetAutoRecordSuccess, meeting_service_server::MeetingService,
};

pub struct MeetingGrpcService {
    pub inner: DomainMeetingService,
}

#[tonic::async_trait]
impl MeetingService for MeetingGrpcService {
    async fn list_meetings(
        &self,
        request: Request<ListMeetingsRequest>,
    ) -> Result<Response<ListMeetingsResponse>, Status> {
        let req = request.into_inner();
        match self.inner.list_meetings(&req.token).await {
            Ok(rows) => Ok(Response::new(ListMeetingsResponse {
                result: Some(proto::list_meetings_response::Result::Success(
                    ListMeetingsSuccess {
                        meetings: rows.iter().map(to_proto_meeting).collect(),
                    },
                )),
            })),
            Err(error) => {
                tracing::warn!(code = error.code(), "gRPC ListMeetings failed");
                Ok(Response::new(ListMeetingsResponse {
                    result: Some(proto::list_meetings_response::Result::Error(
                        to_proto_error(&error),
                    )),
                }))
            }
        }
    }

    async fn get_meeting(
        &self,
        request: Request<GetMeetingRequest>,
    ) -> Result<Response<GetMeetingResponse>, Status> {
        let req = request.into_inner();
        match self.inner.get_meeting(&req.token, &req.meeting_id).await {
            Ok(row) => Ok(Response::new(GetMeetingResponse {
                result: Some(proto::get_meeting_response::Result::Success(
                    to_proto_meeting(&row),
                )),
            })),
            Err(error) => Ok(Response::new(GetMeetingResponse {
                result: Some(proto::get_meeting_response::Result::Error(to_proto_error(
                    &error,
                ))),
            })),
        }
    }

    async fn set_auto_record(
        &self,
        request: Request<SetAutoRecordRequest>,
    ) -> Result<Response<SetAutoRecordResponse>, Status> {
        let req = request.into_inner();
        match self
            .inner
            .set_auto_record(&req.token, &req.meeting_id, req.auto_record)
            .await
        {
            Ok(_) => Ok(Response::new(SetAutoRecordResponse {
                result: Some(proto::set_auto_record_response::Result::Success(
                    SetAutoRecordSuccess { ok: true },
                )),
            })),
            Err(error) => Ok(Response::new(SetAutoRecordResponse {
                result: Some(proto::set_auto_record_response::Result::Error(
                    to_proto_error(&error),
                )),
            })),
        }
    }

    async fn get_dashboard_summary(
        &self,
        request: Request<GetDashboardSummaryRequest>,
    ) -> Result<Response<GetDashboardSummaryResponse>, Status> {
        let req = request.into_inner();
        match self.inner.dashboard_summary(&req.token).await {
            Ok(summary) => Ok(Response::new(GetDashboardSummaryResponse {
                result: Some(proto::get_dashboard_summary_response::Result::Success(
                    DashboardSummary {
                        upcoming_7d: summary.upcoming_7d,
                        connected_calendars: summary.connected_calendars,
                        completed_recordings: summary.completed_recordings,
                        upcoming_meetings: summary
                            .upcoming_meetings
                            .iter()
                            .map(to_proto_meeting)
                            .collect(),
                    },
                )),
            })),
            Err(error) => Ok(Response::new(GetDashboardSummaryResponse {
                result: Some(proto::get_dashboard_summary_response::Result::Error(
                    to_proto_error(&error),
                )),
            })),
        }
    }
}

fn to_proto_meeting(row: &crate::db::meetings::MeetingRow) -> Meeting {
    let (attendees, calendar_id, calendar_label) = crate::meeting::row_to_proto_fields(row);
    Meeting {
        id: row.id.clone(),
        title: row.title.clone(),
        meet_url: row.meet_url.clone(),
        start_at: row.start_at,
        end_at: row.end_at,
        auto_record: row.auto_join != 0,
        calendar_id,
        calendar_label,
        attendees,
        description: row.description.clone().unwrap_or_default(),
    }
}

fn to_proto_error(error: &MeetingError) -> ProtoMeetingError {
    ProtoMeetingError {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}
