import type { CalendarConnectErrorCode, CalendarProvider } from "./constants"

export type {
  BotRunStatus,
  CalendarConnectErrorCode,
  CalendarProvider,
} from "./constants"
export {
  BOT_RUN_STATUSES,
  CALENDAR_CONNECT_ERROR_CODES,
  CALENDAR_PROVIDERS,
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

export type { MeetingTimelineEventPayload } from "./timeline"

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
