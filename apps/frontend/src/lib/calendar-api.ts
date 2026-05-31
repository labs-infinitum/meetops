import type { CalendarConnectError, CalendarConnection, CalendarProvider } from "@/types"
import { isCalendarConnectErrorCode, isCalendarProvider } from "@/types/constants"
import { formatGrpcApiError, GrpcApiError, unwrapGrpcOneof } from "@/lib/api-errors"
import { getCalendarClient } from "@/lib/grpc-client"
import { protoInt64 } from "@/lib/proto-fields"
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

export class CalendarApiError extends GrpcApiError {
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
    super(code, message, "CalendarApiError")
    this.email = extras?.email
    this.provider = extras?.provider
    this.otherAccountHint = extras?.otherAccountHint
  }

  toConnectError(): CalendarConnectError {
    const provider = this.provider ?? "google"
    if (isCalendarConnectErrorCode(this.code)) {
      return buildConnectError(this.code, provider, {
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
    provider: isCalendarProvider(row.provider) ? row.provider : "google",
    email: row.email,
    label: row.label,
    connected: row.connected,
    lastSyncedAt: protoInt64(row.lastSyncedAt, true),
    meetingCount: row.meetingCount,
    color: row.color,
  }
}

function parseCalendarError(error: CalendarError): CalendarApiError {
  return new CalendarApiError(error.code, error.message, {
    email: error.email || undefined,
    provider: isCalendarProvider(error.provider) ? error.provider : "google",
    otherAccountHint: error.otherAccountHint || undefined,
  })
}

function unwrapListResponse(response: ListConnectionsResponse): CalendarConnection[] {
  return unwrapGrpcOneof(response.result, {
    success: (s) => s.connections.map(mapConnection),
    error: parseCalendarError,
    emptyMessage: "Empty calendar list response.",
    EmptyError: CalendarApiError,
  })
}

function unwrapStartOAuth(response: StartOAuthResponse): string {
  return unwrapGrpcOneof(response.result, {
    success: (s) => s.redirectUrl,
    error: parseCalendarError,
    emptyMessage: "Empty StartOAuth response.",
    EmptyError: CalendarApiError,
  })
}

function unwrapDisconnect(response: DisconnectResponse): boolean {
  return unwrapGrpcOneof(response.result, {
    success: (s) => s.ok,
    error: parseCalendarError,
    emptyMessage: "Empty disconnect response.",
    EmptyError: CalendarApiError,
  })
}

function unwrapSyncNow(response: SyncNowResponse): {
  seen: number
  upserted: number
  deleted: number
  lastSyncedAt: number
} {
  return unwrapGrpcOneof(response.result, {
    success: (success) => ({
      seen: success.seen,
      upserted: success.upserted,
      deleted: success.deleted,
      lastSyncedAt: Number(success.lastSyncedAt),
    }),
    error: parseCalendarError,
    emptyMessage: "Empty sync response.",
    EmptyError: CalendarApiError,
  })
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
  return formatGrpcApiError(error, () => null, "Calendar request failed.")
}

export function parseOAuthCallbackParams(search: string): CalendarConnectError | null {
  const params = new URLSearchParams(search)
  const oauth = params.get("oauth")
  if (!oauth) return null

  const providerParam = params.get("provider") ?? "google"
  const provider = isCalendarProvider(providerParam) ? providerParam : "google"
  const email = params.get("email") ?? undefined
  const otherHint = params.get("other_hint") ?? undefined

  if (oauth === "connected") {
    return null
  }

  if (isCalendarConnectErrorCode(oauth)) {
    const code = oauth
    return buildConnectError(code, provider, {
      email,
      otherAccountHint: otherHint,
    })
  }

  return buildConnectError("oauth_failed", provider, { email })
}
