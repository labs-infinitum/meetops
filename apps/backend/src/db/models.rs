#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EligibleMeeting {
    pub id: String,
    pub meet_url: String,
    pub start_at: i64,
    pub end_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotRunStatus {
    Queued,
    Joining,
    InMeeting,
    Recording,
    Completed,
    Failed,
}

impl BotRunStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Joining => "joining",
            Self::InMeeting => "in_meeting",
            Self::Recording => "recording",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    /// Parse a DB/API status string (proto `BotRun.status` wire values).
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(Self::Queued),
            "joining" => Some(Self::Joining),
            "in_meeting" => Some(Self::InMeeting),
            "recording" => Some(Self::Recording),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Joining | Self::InMeeting | Self::Recording
        )
    }

    pub fn is_active_str(status: &str) -> bool {
        Self::from_db_str(status).is_some_and(Self::is_active)
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BotRunListItem {
    pub id: i64,
    pub meeting_id: String,
    pub title: String,
    pub status: String,
    pub error: Option<String>,
    pub meeting_start_at: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub created_at: i64,
    pub recording_path: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MeetingTimelineEvent {
    pub id: i64,
    pub bot_run_id: i64,
    pub event_type: String,
    pub occurred_at: i64,
    pub relative_ms: i64,
    pub participant_id: Option<String>,
    pub participant_name: Option<String>,
    pub payload_json: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewMeetingTimelineEvent {
    pub event_type: String,
    pub occurred_at: i64,
    pub relative_ms: i64,
    pub participant_id: Option<String>,
    pub participant_name: Option<String>,
    pub payload_json: Option<String>,
}
