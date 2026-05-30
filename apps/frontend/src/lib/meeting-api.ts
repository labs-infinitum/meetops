import type { Meeting } from "@/types"
import { getMeetingClient } from "@/lib/grpc-client"
import { withAuthGuard } from "@/lib/session-guard"
import type {
  DashboardSummary,
  GetDashboardSummaryResponse,
  GetMeetingResponse,
  ListMeetingsResponse,
  Meeting as ProtoMeeting,
  MeetingError,
  SetAutoRecordResponse,
} from "@/gen/notetaker/v1/meeting"

export class MeetingApiError extends Error {
  code: string

  constructor(code: string, message: string) {
    super(message)
    this.code = code
    this.name = "MeetingApiError"
  }
}

function mapMeeting(row: ProtoMeeting): Meeting {
  return {
    id: row.id,
    title: row.title,
    meetUrl: row.meetUrl,
    startAt: Number(row.startAt),
    endAt: Number(row.endAt),
    autoRecord: row.autoRecord,
    calendarId: row.calendarId,
    attendees: [...row.attendees],
    description: row.description || undefined,
    calendarLabel: row.calendarLabel || undefined,
  }
}

function parseError(error: MeetingError): MeetingApiError {
  return new MeetingApiError(error.code, error.message)
}

function unwrapList(response: ListMeetingsResponse): Meeting[] {
  if (response.result.oneofKind === "success") {
    return response.result.success.meetings.map(mapMeeting)
  }
  if (response.result.oneofKind === "error") {
    throw parseError(response.result.error)
  }
  throw new MeetingApiError("internal_error", "Empty meetings list response.")
}

function unwrapGet(response: GetMeetingResponse): Meeting {
  if (response.result.oneofKind === "success") {
    return mapMeeting(response.result.success)
  }
  if (response.result.oneofKind === "error") {
    throw parseError(response.result.error)
  }
  throw new MeetingApiError("internal_error", "Empty meeting response.")
}

function unwrapSetAutoRecord(response: SetAutoRecordResponse): boolean {
  if (response.result.oneofKind === "success") {
    return response.result.success.ok
  }
  if (response.result.oneofKind === "error") {
    throw parseError(response.result.error)
  }
  throw new MeetingApiError("internal_error", "Empty set auto-record response.")
}

function unwrapDashboard(response: GetDashboardSummaryResponse): {
  upcoming7d: number
  connectedCalendars: number
  completedRecordings: number
  upcomingMeetings: Meeting[]
} {
  if (response.result.oneofKind === "success") {
    const summary: DashboardSummary = response.result.success
    return {
      upcoming7d: summary.upcoming7D,
      connectedCalendars: summary.connectedCalendars,
      completedRecordings: summary.completedRecordings,
      upcomingMeetings: summary.upcomingMeetings.map(mapMeeting),
    }
  }
  if (response.result.oneofKind === "error") {
    throw parseError(response.result.error)
  }
  throw new MeetingApiError("internal_error", "Empty dashboard response.")
}

export async function apiListMeetings(token: string): Promise<Meeting[]> {
  return withAuthGuard(async () => {
    const client = getMeetingClient()
    const { response } = await client.listMeetings({ token })
    return unwrapList(response)
  })
}

export async function apiGetMeeting(token: string, meetingId: string): Promise<Meeting> {
  return withAuthGuard(async () => {
    const client = getMeetingClient()
    const { response } = await client.getMeeting({ token, meetingId })
    return unwrapGet(response)
  })
}

export async function apiSetAutoRecord(
  token: string,
  meetingId: string,
  autoRecord: boolean,
): Promise<boolean> {
  return withAuthGuard(async () => {
    const client = getMeetingClient()
    const { response } = await client.setAutoRecord({ token, meetingId, autoRecord })
    return unwrapSetAutoRecord(response)
  })
}

export async function apiGetDashboardSummary(token: string) {
  return withAuthGuard(async () => {
    const client = getMeetingClient()
    const { response } = await client.getDashboardSummary({ token })
    return unwrapDashboard(response)
  })
}

export function meetingPath(id: string) {
  return `/app/meetings/${encodeURIComponent(id)}`
}

export function meetingErrorMessage(error: unknown): string {
  if (error instanceof MeetingApiError) {
    if (error.code === "session_not_found") {
      return "Your session has expired. Please sign in again."
    }
    if (error.code === "not_found") {
      return "Meeting not found."
    }
    return error.message
  }
  if (error instanceof Error) {
    if (error.message.includes("Failed to fetch") || error.message.includes("NetworkError")) {
      return "Cannot reach the server. Is the backend running?"
    }
    return error.message
  }
  return "Could not load meetings."
}
