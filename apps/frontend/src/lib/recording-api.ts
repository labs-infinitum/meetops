import type { BotRun, GeneratedNote, MeetingTimelineEvent } from "@/types"
import { isBotRunStatus, isMeetingTimelineEventType } from "@/types"
import { getRecordingClient } from "@/lib/grpc-client"
import { parseTimelinePayloadJson } from "@/lib/timeline-payload"
import { protoInt64, protoOptionalString } from "@/lib/proto-fields"
import { formatGrpcApiError, GrpcApiError, unwrapGrpcOneof } from "@/lib/api-errors"
import { withAuthGuard, SESSION_EXPIRED_MESSAGE } from "@/lib/session-guard"
import type {
  BotRun as ProtoBotRun,
  GeneratedNote as ProtoGeneratedNote,
  GetBotRunResponse,
  GetGeneratedNoteResponse,
  GetPlaybackUrlResponse,
  ListBotRunsResponse,
  ListGeneratedNotesResponse,
  ListMeetingTimelineEventsResponse,
  MeetingTimelineEvent as ProtoMeetingTimelineEvent,
  RecordingError,
  StartAdHocRecordingResponse,
  StopRecordingResponse,
} from "@ai-meeting-notetaker/proto/notetaker/v1/recording"

export class RecordingApiError extends GrpcApiError {
  constructor(code: string, message: string) {
    super(code, message, "RecordingApiError")
  }
}

function mapRun(row: ProtoBotRun): BotRun {
  return {
    id: protoInt64(row.id)!,
    meetingId: row.meetingId,
    meetingTitle: row.meetingTitle,
    status: isBotRunStatus(row.status) ? row.status : "failed",
    error: protoOptionalString(row.error),
    meetingStartAt: protoInt64(row.meetingStartAt)!,
    startedAt: protoInt64(row.startedAt, true),
    endedAt: protoInt64(row.endedAt, true),
    createdAt: protoInt64(row.createdAt)!,
    recordingPath: protoOptionalString(row.recordingPath),
    durationSecs: protoInt64(row.durationSecs, true),
  }
}

function mapTimelineEvent(row: ProtoMeetingTimelineEvent): MeetingTimelineEvent | null {
  if (!isMeetingTimelineEventType(row.eventType)) {
    return null
  }
  const eventType = row.eventType
  return {
    id: protoInt64(row.id)!,
    botRunId: protoInt64(row.botRunId)!,
    eventType,
    occurredAt: protoInt64(row.occurredAt)!,
    relativeMs: protoInt64(row.relativeMs)!,
    participantId: protoOptionalString(row.participantId),
    participantName: protoOptionalString(row.participantName),
    payload: parseTimelinePayloadJson(eventType, row.payloadJson),
    createdAt: protoInt64(row.createdAt)!,
  }
}

function mapGeneratedNote(row: ProtoGeneratedNote): GeneratedNote {
  return {
    transcriptId: protoInt64(row.transcriptId)!,
    botRunId: protoInt64(row.botRunId)!,
    meetingId: row.meetingId,
    meetingTitle: row.meetingTitle,
    recordedAt: protoInt64(row.recordedAt)!,
    transcriptStatus: row.transcriptStatus,
    summaryStatus: protoOptionalString(row.summaryStatus),
    tldr: protoOptionalString(row.tldr),
    summaryMd: protoOptionalString(row.summaryMd),
    rawTranscript: protoOptionalString(row.rawTranscript),
    cleanedTranscript: protoOptionalString(row.cleanedTranscript),
    actionItems: row.actionItems.map((item) => ({
      text: item.text,
      owner: protoOptionalString(item.owner),
      due: protoOptionalString(item.due),
    })),
    error: protoOptionalString(row.error),
  }
}

function parseError(error: RecordingError): RecordingApiError {
  return new RecordingApiError(error.code, error.message)
}

function unwrapList(response: ListBotRunsResponse): BotRun[] {
  return unwrapGrpcOneof(response.result, {
    success: (s) => s.runs.map(mapRun),
    error: parseError,
    emptyMessage: "Empty recordings list response.",
    EmptyError: RecordingApiError,
  })
}

function unwrapGet(response: GetBotRunResponse): BotRun {
  return unwrapGrpcOneof(response.result, {
    success: mapRun,
    error: parseError,
    emptyMessage: "Empty recording response.",
    EmptyError: RecordingApiError,
  })
}

function unwrapPlayback(response: GetPlaybackUrlResponse): { url: string; expiresAt: number } {
  return unwrapGrpcOneof(response.result, {
    success: (s) => ({
      url: s.url,
      expiresAt: Number(s.expiresAt),
    }),
    error: parseError,
    emptyMessage: "Empty playback URL response.",
    EmptyError: RecordingApiError,
  })
}

function unwrapTimeline(response: ListMeetingTimelineEventsResponse): MeetingTimelineEvent[] {
  return unwrapGrpcOneof(response.result, {
    success: (s) =>
      s.events
        .map(mapTimelineEvent)
        .filter((event): event is MeetingTimelineEvent => event !== null),
    error: parseError,
    emptyMessage: "Empty meeting timeline response.",
    EmptyError: RecordingApiError,
  })
}

function unwrapGeneratedNotes(response: ListGeneratedNotesResponse): GeneratedNote[] {
  return unwrapGrpcOneof(response.result, {
    success: (s) => s.notes.map(mapGeneratedNote),
    error: parseError,
    emptyMessage: "Empty generated notes response.",
    EmptyError: RecordingApiError,
  })
}

function unwrapGeneratedNote(response: GetGeneratedNoteResponse): GeneratedNote {
  return unwrapGrpcOneof(response.result, {
    success: mapGeneratedNote,
    error: parseError,
    emptyMessage: "Empty generated note response.",
    EmptyError: RecordingApiError,
  })
}

function unwrapStartAdHoc(response: StartAdHocRecordingResponse): BotRun {
  return unwrapGrpcOneof(response.result, {
    success: (s) => {
      const run = s.run
      if (run) return mapRun(run)
      return mapRun({
        id: s.runId,
        meetingId: "",
        meetingTitle: "",
        status: "queued",
        error: "",
        meetingStartAt: BigInt(0),
        startedAt: BigInt(0),
        endedAt: BigInt(0),
        createdAt: BigInt(0),
        recordingPath: "",
        durationSecs: BigInt(0),
      })
    },
    error: parseError,
    emptyMessage: "Empty ad-hoc recording response.",
    EmptyError: RecordingApiError,
  })
}

function unwrapStop(response: StopRecordingResponse): void {
  unwrapGrpcOneof(response.result, {
    success: () => undefined,
    error: parseError,
    emptyMessage: "Empty stop recording response.",
    EmptyError: RecordingApiError,
  })
}

export async function apiListBotRuns(token: string, limit = 50): Promise<BotRun[]> {
  return withAuthGuard(async () => {
    const client = getRecordingClient()
    const { response } = await client.listBotRuns({ token, limit })
    return unwrapList(response)
  })
}

export async function apiGetBotRun(token: string, runId: number): Promise<BotRun> {
  return withAuthGuard(async () => {
    const client = getRecordingClient()
    const { response } = await client.getBotRun({ token, runId: BigInt(runId) })
    return unwrapGet(response)
  })
}

export async function apiGetPlaybackUrl(
  token: string,
  runId: number,
): Promise<{ url: string; expiresAt: number }> {
  return withAuthGuard(async () => {
    const client = getRecordingClient()
    const { response } = await client.getPlaybackUrl({ token, runId: BigInt(runId) })
    return unwrapPlayback(response)
  })
}

export async function apiListMeetingTimelineEvents(
  token: string,
  runId: number,
  limit = 500,
): Promise<MeetingTimelineEvent[]> {
  return withAuthGuard(async () => {
    const client = getRecordingClient()
    const { response } = await client.listMeetingTimelineEvents({
      token,
      runId: BigInt(runId),
      limit,
    })
    return unwrapTimeline(response)
  })
}

export async function apiListGeneratedNotes(token: string, limit = 50): Promise<GeneratedNote[]> {
  return withAuthGuard(async () => {
    const client = getRecordingClient()
    const { response } = await client.listGeneratedNotes({ token, limit })
    return unwrapGeneratedNotes(response)
  })
}

export async function apiGetGeneratedNote(
  token: string,
  transcriptId: number,
): Promise<GeneratedNote> {
  return withAuthGuard(async () => {
    const client = getRecordingClient()
    const { response } = await client.getGeneratedNote({
      token,
      transcriptId: BigInt(transcriptId),
    })
    return unwrapGeneratedNote(response)
  })
}

export async function apiStartAdHocRecording(token: string, meetUrl: string): Promise<BotRun> {
  return withAuthGuard(async () => {
    const client = getRecordingClient()
    const { response } = await client.startAdHocRecording({ token, meetUrl })
    return unwrapStartAdHoc(response)
  })
}

export async function apiStopRecording(token: string, runId: number): Promise<void> {
  return withAuthGuard(async () => {
    const client = getRecordingClient()
    const { response } = await client.stopRecording({ token, runId: BigInt(runId) })
    unwrapStop(response)
  })
}

export function recordingErrorMessage(error: unknown): string {
  return formatGrpcApiError(
    error,
    (code) => {
      switch (code) {
        case "session_not_found":
          return SESSION_EXPIRED_MESSAGE
        case "not_found":
          return "Recording not found."
        case "not_available":
          return "Recording is not available for playback yet."
        case "invalid_meet_url":
          return "Enter a valid Google Meet link (https://meet.google.com/…)."
        case "bot_busy":
          return "The bot is already recording another meeting. Try again when it finishes."
        case "shutting_down":
          return "The server is shutting down and cannot start new recordings."
        case "not_active":
          return "This recording is not active anymore."
        default:
          return null
      }
    },
    "Could not load recordings.",
  )
}
