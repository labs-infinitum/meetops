import type { User as ProtoUser } from "@ai-meeting-notetaker/proto/notetaker/v1/auth"
import type { CalendarConnection as ProtoCalendarConnection } from "@ai-meeting-notetaker/proto/notetaker/v1/calendar"
import type { Meeting as ProtoMeeting } from "@ai-meeting-notetaker/proto/notetaker/v1/meeting"
import type {
  BotRun as ProtoBotRun,
  MeetingTimelineEvent as ProtoMeetingTimelineEvent,
} from "@ai-meeting-notetaker/proto/notetaker/v1/recording"

import type { BotRunStatus, CalendarProvider } from "./constants"
import type { MeetingTimelineEventPayload } from "./timeline"

/** Replace protobuf `int64` (`bigint`) fields with `number` for app/UI use. */
type Int64AsNumber<T, K extends keyof T> = Omit<T, K> & { [P in K]: number }

/** Normalized meeting from `notetaker.v1.Meeting` (proto is source of field names). */
export type Meeting = Int64AsNumber<
  Omit<ProtoMeeting, "calendarLabel" | "description">,
  "startAt" | "endAt"
> & {
  calendarLabel?: string
  description?: string
}

/** Normalized bot run from `notetaker.v1.BotRun`. */
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
  "payloadJson" | "participantId" | "participantName"
> & {
  participantId?: string
  participantName?: string
  payload?: MeetingTimelineEventPayload
}

/** Normalized calendar connection from `notetaker.v1.CalendarConnection`. */
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
