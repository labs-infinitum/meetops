/** Payload shapes emitted by the in-meet timeline collector (see backend bot chromium script). */

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
  presenterName: string
  scrollTarget?: string | null
}

export type ScreenShareStoppedPayload = {
  source: string
  presenterName: string
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
