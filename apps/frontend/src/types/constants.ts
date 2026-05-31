/** Wire values for `BotRun.status` (proto string, DB, Rust `BotRunStatus::as_str`). */
export const BOT_RUN_STATUSES = [
  "queued",
  "joining",
  "in_meeting",
  "recording",
  "completed",
  "failed",
] as const

export type BotRunStatus = (typeof BOT_RUN_STATUSES)[number]

export const CALENDAR_PROVIDERS = ["google", "outlook", "apple", "caldav"] as const

export type CalendarProvider = (typeof CALENDAR_PROVIDERS)[number]

/** Calendar OAuth / connect error codes returned by the API and OAuth callback. */
export const CALENDAR_CONNECT_ERROR_CODES = [
  "already_connected_by_other_user",
  "already_connected_by_you",
  "oauth_denied",
  "oauth_failed",
  "invalid_account",
  "provider_unavailable",
  "sync_failed",
] as const

export type CalendarConnectErrorCode = (typeof CALENDAR_CONNECT_ERROR_CODES)[number]

export function isBotRunStatus(value: string): value is BotRunStatus {
  return (BOT_RUN_STATUSES as readonly string[]).includes(value)
}

export function isCalendarProvider(value: string): value is CalendarProvider {
  return (CALENDAR_PROVIDERS as readonly string[]).includes(value)
}

export function isCalendarConnectErrorCode(value: string): value is CalendarConnectErrorCode {
  return (CALENDAR_CONNECT_ERROR_CODES as readonly string[]).includes(value)
}
