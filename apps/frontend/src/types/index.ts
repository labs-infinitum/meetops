import type { CalendarConnectErrorCode, CalendarProvider } from "./constants"

export type {
  BotRunStatus,
  CalendarConnectErrorCode,
  CalendarProvider,
} from "./constants"
export {
  isActiveBotRun,
  isBotRunStatus,
  isCalendarConnectErrorCode,
  isCalendarProvider,
} from "./constants"

export type {
  AuthSession,
  BotRun,
  CalendarConnection,
  Meeting,
  MeetingTimelineEvent,
  User,
} from "./domain"

export type {
  ActiveSpeakerChangedPayload,
  CollectorWarningPayload,
  MeetingTimelineEventPayload,
  MeetingTimelineEventType,
  ParticipantJoinedPayload,
  ParticipantLeftPayload,
  ParticipantSnapshotPayload,
  ScreenShareStartedPayload,
  ScreenShareStoppedPayload,
  TimelineParticipantRef,
} from "./timeline"
export { isMeetingTimelineEventType } from "./timeline"

/** UI-enriched calendar connect error (titles/messages); codes align with `CalendarError` proto. */
export type CalendarConnectError = {
  code: CalendarConnectErrorCode
  title: string
  message: string
  email?: string
  provider: CalendarProvider
  otherAccountHint?: string
}

/** UI-only types (no proto yet). */
export type ActionItem = {
  id: string
  text: string
  assignee?: string
  dueDate?: number
  completed: boolean
  meetingId: string
  meetingTitle: string
}

export type Note = {
  id: string
  meetingId: string
  meetingTitle: string
  recordedAt: number
  tldr: string
  summaryMd: string
  actionItems: ActionItem[]
  tags: string[]
}
