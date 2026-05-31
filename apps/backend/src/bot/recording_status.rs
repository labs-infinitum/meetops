use std::fmt;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RecordingStatus {
    WaitingForClick,
    NotInstalled,
    NotStarted {
        #[serde(default)]
        errors: Vec<String>,
    },
    Starting,
    Recording {
        #[serde(default, rename = "mimeType")]
        mime_type: Option<String>,
        #[serde(default, rename = "audioTrackCount")]
        audio_track_count: Option<u64>,
        #[serde(default, rename = "videoTrackCount")]
        video_track_count: Option<u64>,
        #[serde(default, rename = "audioSource")]
        audio_source: Option<String>,
    },
    Failed {
        name: String,
        message: String,
    },
    Stopped {
        #[serde(default)]
        errors: Vec<String>,
    },
}

impl RecordingStatus {
    pub fn phase(&self) -> RecordingPhase {
        match self {
            Self::Recording { .. } => RecordingPhase::Recording,
            Self::Failed { .. } => RecordingPhase::Failed,
            Self::Stopped { .. } | Self::NotStarted { .. } => RecordingPhase::Stopped,
            Self::Starting => RecordingPhase::Starting,
            Self::WaitingForClick | Self::NotInstalled => RecordingPhase::Pending,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingPhase {
    Pending,
    Starting,
    Recording,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecordingRuntimeStatus {
    #[serde(flatten)]
    pub status: RecordingStatus,
    #[serde(default, rename = "chunks")]
    pub chunk_count: u64,
    #[serde(default, rename = "uploadedBytes")]
    pub uploaded_bytes: u64,
    #[serde(default, rename = "acceptedChunks")]
    pub accepted_chunks: u64,
    #[serde(default, rename = "uploadErrors")]
    pub upload_errors: Vec<String>,
    #[serde(default, rename = "bindingAvailable")]
    pub binding_available: bool,
    #[serde(default, rename = "lastParticipantCount")]
    pub last_participant_count: Option<u64>,
    #[serde(default, rename = "lastParticipantSource")]
    pub last_participant_source: Option<String>,
    #[serde(default, rename = "audioTrackCount")]
    pub audio_track_count: Option<u64>,
    #[serde(default, rename = "videoTrackCount")]
    pub video_track_count: Option<u64>,
    #[serde(default, rename = "audioSource")]
    pub audio_source: Option<String>,
}

impl fmt::Display for RecordingRuntimeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "phase={:?} chunks={} uploaded_bytes={} accepted_chunks={} upload_errors={} binding={} audio_tracks={:?} video_tracks={:?} audio_source={:?} participants={:?} participant_source={:?}",
            self.status.phase(),
            self.chunk_count,
            self.uploaded_bytes,
            self.accepted_chunks,
            self.upload_errors.len(),
            self.binding_available,
            self.audio_track_count.or_else(|| match &self.status {
                RecordingStatus::Recording {
                    audio_track_count, ..
                } => *audio_track_count,
                _ => None,
            }),
            self.video_track_count.or_else(|| match &self.status {
                RecordingStatus::Recording {
                    video_track_count, ..
                } => *video_track_count,
                _ => None,
            }),
            self.audio_source.as_deref().or_else(|| match &self.status {
                RecordingStatus::Recording { audio_source, .. } => audio_source.as_deref(),
                _ => None,
            }),
            self.last_participant_count,
            self.last_participant_source,
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecordingBindingPayload {
    Chunk {
        size: u64,
        data: String,
        #[serde(default, rename = "participantCount")]
        participant_count: Option<u64>,
        #[serde(default, rename = "participantSource")]
        participant_source: Option<String>,
    },
    Done,
}

#[cfg(test)]
mod tests {
    use super::{RecordingBindingPayload, RecordingRuntimeStatus, RecordingStatus};

    #[test]
    fn deserializes_recording_status_variants() {
        let recording: RecordingStatus = serde_json::from_str(
            r#"{"status":"recording","mimeType":"video/webm","audioTrackCount":1,"videoTrackCount":1,"audioSource":"meet_element_audio"}"#,
        )
        .unwrap();
        assert!(matches!(recording, RecordingStatus::Recording { .. }));

        let failed: RecordingStatus = serde_json::from_str(
            r#"{"status":"failed","name":"NotAllowedError","message":"Permission denied"}"#,
        )
        .unwrap();
        assert!(matches!(failed, RecordingStatus::Failed { .. }));
    }

    #[test]
    fn deserializes_runtime_status_with_counters() {
        let status: RecordingRuntimeStatus = serde_json::from_str(
            r#"{"status":"recording","mimeType":"video/webm","chunks":3,"uploadedBytes":4096,"acceptedChunks":3,"uploadErrors":[],"bindingAvailable":true,"lastParticipantCount":2,"lastParticipantSource":"tiles"}"#,
        )
        .unwrap();
        assert_eq!(status.chunk_count, 3);
        assert_eq!(status.uploaded_bytes, 4096);
        assert_eq!(status.last_participant_count, Some(2));
    }

    #[test]
    fn deserializes_binding_chunk_and_done() {
        let chunk: RecordingBindingPayload = serde_json::from_str(
            r#"{"kind":"chunk","sequence":1,"size":12,"data":"YWJj","participantCount":2,"participantSource":"tiles"}"#,
        )
        .unwrap();
        assert!(matches!(chunk, RecordingBindingPayload::Chunk { .. }));

        let done: RecordingBindingPayload = serde_json::from_str(r#"{"kind":"done"}"#).unwrap();
        assert!(matches!(done, RecordingBindingPayload::Done));
    }

    #[test]
    fn deserializes_stop_recording_status() {
        let status: RecordingStatus =
            serde_json::from_str(r#"{"status":"stopped","errors":[]}"#).unwrap();
        assert!(matches!(status, RecordingStatus::Stopped { .. }));
    }
}
