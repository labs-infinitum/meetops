import type { CalendarConnectError, CalendarConnection, CalendarProvider } from "@/types"
import { getCalendarClient } from "@/lib/grpc-client"
import { buildConnectError } from "@/lib/calendar-connect"
import { withAuthGuard } from "@/lib/session-guard"
import type {
  CalendarConnection as ProtoConnection,
  CalendarError,
  ListConnectionsResponse,
  StartOAuthResponse,
  DisconnectResponse,
  SyncNowResponse,
} from "@/gen/notetaker/v1/calendar"

export class CalendarApiError extends Error {
  code: string
  email?: string
  provider?: CalendarProvider
  otherAccountHint?: string

  constructor(
    code: string,
    message: string,
    extras?: {
      email?: string
      provider?: CalendarProvider
      otherAccountHint?: string
    },
  ) {
    super(message)
    this.code = code
    this.name = "CalendarApiError"
    this.email = extras?.email
    this.provider = extras?.provider
    this.otherAccountHint = extras?.otherAccountHint
  }

  toConnectError(): CalendarConnectError {
    const provider = this.provider ?? "google"
    const code = this.code as CalendarConnectError["code"]
    if (
      code === "already_connected_by_other_user" ||
      code === "already_connected_by_you" ||
      code === "oauth_denied" ||
      code === "oauth_failed" ||
      code === "invalid_account" ||
      code === "provider_unavailable" ||
      code === "sync_failed"
    ) {
      return buildConnectError(code, provider, {
        email: this.email,
        otherAccountHint: this.otherAccountHint,
      })
    }
    return buildConnectError("oauth_failed", provider, { email: this.email })
  }
}

function mapConnection(row: ProtoConnection): CalendarConnection {
  return {
    id: row.id,
    provider: row.provider as CalendarProvider,
    email: row.email,
    label: row.label,
    connected: row.connected,
    lastSyncedAt: row.lastSyncedAt ? Number(row.lastSyncedAt) : undefined,
    meetingCount: row.meetingCount,
    color: row.color,
  }
}

function parseCalendarError(error: CalendarError): CalendarApiError {
  return new CalendarApiError(error.code, error.message, {
    email: error.email || undefined,
    provider: (error.provider || "google") as CalendarProvider,
    otherAccountHint: error.otherAccountHint || undefined,
  })
}

function unwrapListResponse(response: ListConnectionsResponse): CalendarConnection[] {
  if (response.result.oneofKind === "success") {
    return response.result.success.connections.map(mapConnection)
  }
  if (response.result.oneofKind === "error") {
    throw parseCalendarError(response.result.error)
  }
  throw new CalendarApiError("internal_error", "Empty calendar list response.")
}

function unwrapStartOAuth(response: StartOAuthResponse): string {
  if (response.result.oneofKind === "success") {
    return response.result.success.redirectUrl
  }
  if (response.result.oneofKind === "error") {
    throw parseCalendarError(response.result.error)
  }
  throw new CalendarApiError("internal_error", "Empty StartOAuth response.")
}

function unwrapDisconnect(response: DisconnectResponse): boolean {
  if (response.result.oneofKind === "success") {
    return response.result.success.ok
  }
  if (response.result.oneofKind === "error") {
    throw parseCalendarError(response.result.error)
  }
  throw new CalendarApiError("internal_error", "Empty disconnect response.")
}

function unwrapSyncNow(response: SyncNowResponse): {
  seen: number
  upserted: number
  deleted: number
  lastSyncedAt: number
} {
  if (response.result.oneofKind === "success") {
    const success = response.result.success
    return {
      seen: success.seen,
      upserted: success.upserted,
      deleted: success.deleted,
      lastSyncedAt: Number(success.lastSyncedAt),
    }
  }
  if (response.result.oneofKind === "error") {
    throw parseCalendarError(response.result.error)
  }
  throw new CalendarApiError("internal_error", "Empty sync response.")
}

export async function apiListConnections(token: string): Promise<CalendarConnection[]> {
  return withAuthGuard(async () => {
    const client = getCalendarClient()
    const { response } = await client.listConnections({ token })
    return unwrapListResponse(response)
  })
}

export async function apiStartGoogleOAuth(
  token: string,
  label?: string,
  color?: string,
): Promise<string> {
  return withAuthGuard(async () => {
    const client = getCalendarClient()
    const { response } = await client.startOAuth({
      token,
      provider: "google",
      label: label ?? "",
      color: color ?? "",
    })
    return unwrapStartOAuth(response)
  })
}

export async function apiDisconnectCalendar(
  token: string,
  connectionId: string,
): Promise<boolean> {
  return withAuthGuard(async () => {
    const client = getCalendarClient()
    const { response } = await client.disconnect({ token, connectionId })
    return unwrapDisconnect(response)
  })
}

export async function apiSyncCalendar(
  token: string,
  connectionId: string,
): Promise<{ seen: number; upserted: number; deleted: number; lastSyncedAt: number }> {
  return withAuthGuard(async () => {
    const client = getCalendarClient()
    const { response } = await client.syncNow({ token, connectionId })
    return unwrapSyncNow(response)
  })
}

export function calendarErrorMessage(error: unknown): string {
  if (error instanceof CalendarApiError) {
    return error.toConnectError().message
  }
  if (error instanceof Error) {
    if (error.message.includes("Failed to fetch") || error.message.includes("NetworkError")) {
      return "Cannot reach the server. Is the backend running?"
    }
    return error.message
  }
  return "Calendar request failed."
}

export function parseOAuthCallbackParams(search: string): CalendarConnectError | null {
  const params = new URLSearchParams(search)
  const oauth = params.get("oauth")
  if (!oauth) return null

  const provider = (params.get("provider") ?? "google") as CalendarProvider
  const email = params.get("email") ?? undefined
  const otherHint = params.get("other_hint") ?? undefined

  if (oauth === "connected") {
    return null
  }

  const code = oauth as CalendarConnectError["code"]
  if (
    code === "already_connected_by_other_user" ||
    code === "already_connected_by_you" ||
    code === "oauth_denied" ||
    code === "oauth_failed"
  ) {
    return buildConnectError(code, provider, {
      email,
      otherAccountHint: otherHint,
    })
  }

  return buildConnectError("oauth_failed", provider, { email })
}
