export type BotRunStatus =
  | "queued"
  | "joining"
  | "in_meeting"
  | "recording"
  | "completed"
  | "failed"

export type CalendarProvider = "google" | "outlook" | "apple" | "caldav"

export type CalendarConnectErrorCode =
  | "already_connected_by_other_user"
  | "already_connected_by_you"
  | "oauth_denied"
  | "oauth_failed"
  | "invalid_account"
  | "provider_unavailable"
  | "sync_failed"

export type CalendarConnectError = {
  code: CalendarConnectErrorCode
  title: string
  message: string
  email?: string
  provider: CalendarProvider
  /** Hint shown when another platform user owns the account */
  otherAccountHint?: string
}

export type CalendarConnection = {
  id: string
  provider: CalendarProvider
  email: string
  label: string
  connected: boolean
  lastSyncedAt?: number
  meetingCount: number
  color: string
}

export type Meeting = {
  id: string
  title: string
  meetUrl: string
  startAt: number
  endAt: number
  autoRecord: boolean
  calendarId: string
  attendees: string[]
  description?: string
}

export type BotRun = {
  id: number
  meetingId: string
  meetingTitle: string
  status: BotRunStatus
  error?: string
  meetingStartAt: number
  startedAt?: number
  endedAt?: number
  createdAt: number
  recordingPath?: string
  durationSecs?: number
}

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

export type User = {
  id: string
  name: string
  email: string
  username: string
  avatarUrl?: string
}

export type AuthSession = {
  user: User
  token: string
}
