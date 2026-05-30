import { GrpcWebFetchTransport } from "@protobuf-ts/grpcweb-transport"
import { AuthServiceClient } from "@/gen/notetaker/v1/auth.client"
import { CalendarServiceClient } from "@/gen/notetaker/v1/calendar.client"
import { MeetingServiceClient } from "@/gen/notetaker/v1/meeting.client"

const grpcBaseUrl = import.meta.env.VITE_GRPC_BASE_URL ?? ""

let authClient: AuthServiceClient | null = null
let calendarClient: CalendarServiceClient | null = null
let meetingClient: MeetingServiceClient | null = null

function createTransport() {
  return new GrpcWebFetchTransport({
    baseUrl: grpcBaseUrl,
    format: "binary",
  })
}

export function getAuthClient(): AuthServiceClient {
  if (!authClient) {
    authClient = new AuthServiceClient(createTransport())
  }
  return authClient
}

export function getCalendarClient(): CalendarServiceClient {
  if (!calendarClient) {
    calendarClient = new CalendarServiceClient(createTransport())
  }
  return calendarClient
}

export function getMeetingClient(): MeetingServiceClient {
  if (!meetingClient) {
    meetingClient = new MeetingServiceClient(createTransport())
  }
  return meetingClient
}
