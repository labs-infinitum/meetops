import { GrpcWebFetchTransport } from "@protobuf-ts/grpcweb-transport"
import { AuthServiceClient } from "@/gen/notetaker/v1/auth.client"
import { CalendarServiceClient } from "@/gen/notetaker/v1/calendar.client"
import { MeetingServiceClient } from "@/gen/notetaker/v1/meeting.client"
import { RecordingServiceClient } from "@/gen/notetaker/v1/recording.client"

const grpcBaseUrl = import.meta.env.VITE_GRPC_BASE_URL ?? ""

let transport: GrpcWebFetchTransport | null = null
let authClient: AuthServiceClient | null = null
let calendarClient: CalendarServiceClient | null = null
let meetingClient: MeetingServiceClient | null = null
let recordingClient: RecordingServiceClient | null = null

function getTransport(): GrpcWebFetchTransport {
  if (!transport) {
    transport = new GrpcWebFetchTransport({
      baseUrl: grpcBaseUrl,
      format: "binary",
    })
  }
  return transport
}

export function getAuthClient(): AuthServiceClient {
  if (!authClient) {
    authClient = new AuthServiceClient(getTransport())
  }
  return authClient
}

export function getCalendarClient(): CalendarServiceClient {
  if (!calendarClient) {
    calendarClient = new CalendarServiceClient(getTransport())
  }
  return calendarClient
}

export function getMeetingClient(): MeetingServiceClient {
  if (!meetingClient) {
    meetingClient = new MeetingServiceClient(getTransport())
  }
  return meetingClient
}

export function getRecordingClient(): RecordingServiceClient {
  if (!recordingClient) {
    recordingClient = new RecordingServiceClient(getTransport())
  }
  return recordingClient
}
