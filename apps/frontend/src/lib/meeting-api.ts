import type { Meeting } from "@/types"
import { getMeetingClient } from "@/lib/grpc-client"
import { protoInt64, protoOptionalString } from "@/lib/proto-fields"
import { formatGrpcApiError, GrpcApiError, unwrapGrpcOneof } from "@/lib/api-errors"
import { withAuthGuard, SESSION_EXPIRED_MESSAGE } from "@/lib/session-guard"
import type {
  DashboardSummary,
  GetDashboardSummaryResponse,
  GetMeetingResponse,
  ListMeetingsResponse,
  Meeting as ProtoMeeting,
  MeetingError,
  SetAutoRecordResponse,
} from "@/gen/notetaker/v1/meeting"

export class MeetingApiError extends GrpcApiError {
  constructor(code: string, message: string) {
    super(code, message, "MeetingApiError")
  }
}

function mapMeeting(row: ProtoMeeting): Meeting {
  return {
    id: row.id,
    title: row.title,
    meetUrl: row.meetUrl,
    startAt: protoInt64(row.startAt)!,
    endAt: protoInt64(row.endAt)!,
    autoRecord: row.autoRecord,
    calendarId: row.calendarId,
    attendees: [...row.attendees],
    description: protoOptionalString(row.description),
    calendarLabel: protoOptionalString(row.calendarLabel),
  }
}

function parseError(error: MeetingError): MeetingApiError {
  return new MeetingApiError(error.code, error.message)
}

function unwrapList(response: ListMeetingsResponse): Meeting[] {
  return unwrapGrpcOneof(response.result, {
    success: (s) => s.meetings.map(mapMeeting),
    error: parseError,
    emptyMessage: "Empty meetings list response.",
    EmptyError: MeetingApiError,
  })
}

function unwrapGet(response: GetMeetingResponse): Meeting {
  return unwrapGrpcOneof(response.result, {
    success: mapMeeting,
    error: parseError,
    emptyMessage: "Empty meeting response.",
    EmptyError: MeetingApiError,
  })
}

function unwrapSetAutoRecord(response: SetAutoRecordResponse): boolean {
  return unwrapGrpcOneof(response.result, {
    success: (s) => s.ok,
    error: parseError,
    emptyMessage: "Empty set auto-record response.",
    EmptyError: MeetingApiError,
  })
}

function unwrapDashboard(response: GetDashboardSummaryResponse): {
  upcoming7d: number
  connectedCalendars: number
  completedRecordings: number
  upcomingMeetings: Meeting[]
} {
  return unwrapGrpcOneof(response.result, {
    success: (summary: DashboardSummary) => ({
      upcoming7d: summary.upcoming7D,
      connectedCalendars: summary.connectedCalendars,
      completedRecordings: summary.completedRecordings,
      upcomingMeetings: summary.upcomingMeetings.map(mapMeeting),
    }),
    error: parseError,
    emptyMessage: "Empty dashboard response.",
    EmptyError: MeetingApiError,
  })
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
  return formatGrpcApiError(
    error,
    (code) => {
      if (code === "session_not_found") {
        return SESSION_EXPIRED_MESSAGE
      }
      if (code === "not_found") {
        return "Meeting not found."
      }
      return null
    },
    "Could not load meetings.",
  )
}
