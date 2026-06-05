import type { User as ProtoUser } from "@ai-meeting-notetaker/proto/notetaker/v1/auth"
import type { CalendarConnection as ProtoCalendarConnection } from "@ai-meeting-notetaker/proto/notetaker/v1/calendar"
import type { Meeting as ProtoMeeting } from "@ai-meeting-notetaker/proto/notetaker/v1/meeting"
import type {
  BotRun as ProtoBotRun,
  MeetingTimelineEvent as ProtoMeetingTimelineEvent,
} from "@ai-meeting-notetaker/proto/notetaker/v1/recording"

import type { BotRunStatus, CalendarProvider } from "./constants"
import type { Int64AsNumber } from "./proto-utils"
import type {
  MeetingTimelineEventPayload,
  MeetingTimelineEventType,
} from "./timeline"

// Proto-derived domain types; int64 timestamps/ids are numbers in the UI.

export type Meeting = Int64AsNumber<
  Omit<ProtoMeeting, "calendarLabel" | "description">,
  "startAt" | "endAt"
> & {
  calendarLabel?: string
  description?: string
}

export type BotRun = Omit<
  Int64AsNumber<
    ProtoBotRun,
    "id" | "meetingStartAt" | "startedAt" | "endedAt" | "createdAt" | "durationSecs"
  >,
  "status" | "error" | "recordingPath" | "startedAt" | "endedAt" | "durationSecs"
> & {
  status: BotRunStatus
  error?: string
  startedAt?: number
  endedAt?: number
  recordingPath?: string
  durationSecs?: number
}

/** Normalized timeline event; `payload` is parsed from proto `payload_json`. */
export type MeetingTimelineEvent = Omit<
  Int64AsNumber<
    ProtoMeetingTimelineEvent,
    "id" | "botRunId" | "occurredAt" | "relativeMs" | "createdAt"
  >,
  "payloadJson" | "participantId" | "participantName" | "eventType"
> & {
  eventType: MeetingTimelineEventType
  participantId?: string
  participantName?: string
  payload?: MeetingTimelineEventPayload
}

export type CalendarConnection = Omit<
  Int64AsNumber<ProtoCalendarConnection, "lastSyncedAt">,
  "lastSyncedAt" | "provider"
> & {
  provider: CalendarProvider
  lastSyncedAt?: number
}

/** Auth user; extends proto with optional UI-only `avatarUrl`. */
export type User = ProtoUser & {
  avatarUrl?: string
}

export type AuthSession = {
  user: User
  token: string
  expiresAt?: number
}
