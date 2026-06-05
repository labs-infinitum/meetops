// Timeline wire types from the in-meet collector (`install_recording_control.js`).

const MEETING_TIMELINE_EVENT_TYPES = [
  "participant_joined",
  "participant_left",
  "screen_share_started",
  "screen_share_stopped",
  "active_speaker_changed",
  "participant_snapshot",
  "collector_warning",
] as const

export type MeetingTimelineEventType = (typeof MEETING_TIMELINE_EVENT_TYPES)[number]

export function isMeetingTimelineEventType(value: string): value is MeetingTimelineEventType {
  return (MEETING_TIMELINE_EVENT_TYPES as readonly string[]).includes(value)
}

export type TimelineParticipantRef = {
  id: string
  name: string
  source: string
}

export type ParticipantJoinedPayload = {
  source: string
  rawText?: string
  ariaLabel?: string
  scrollTarget?: string | null
}

export type ParticipantLeftPayload = {
  source: string
}

export type ScreenShareStartedPayload = {
  source: string
  presentationId: string
  presenterParticipantId: string | null
  presenterName: string
  presenterMatch?: string | null
  presenterCandidateIds?: string[]
  scrollTarget?: string | null
}

export type ScreenShareStoppedPayload = {
  source: string
  presentationId: string
  presenterParticipantId: string | null
  presenterName: string
  presenterMatch?: string | null
  presenterCandidateIds?: string[]
}

export type ActiveSpeakerChangedPayload = {
  source: string
}

export type ParticipantSnapshotPayload = {
  count: number
  participants?: TimelineParticipantRef[]
  activeSpeakerId?: string | null
}

export type CollectorWarningPayload = {
  message: string
}

export type MeetingTimelineEventPayload =
  | ParticipantJoinedPayload
  | ParticipantLeftPayload
  | ScreenShareStartedPayload
  | ScreenShareStoppedPayload
  | ActiveSpeakerChangedPayload
  | ParticipantSnapshotPayload
  | CollectorWarningPayload
