import type { BotRun, BotRunStatus } from "@/types"
import { getRecordingClient } from "@/lib/grpc-client"
import type {
  BotRun as ProtoBotRun,
  GetBotRunResponse,
  GetPlaybackUrlResponse,
  ListBotRunsResponse,
  RecordingError,
  StartAdHocRecordingResponse,
} from "@/gen/notetaker/v1/recording"

export class RecordingApiError extends Error {
  code: string

  constructor(code: string, message: string) {
    super(message)
    this.code = code
    this.name = "RecordingApiError"
  }
}

function mapRun(row: ProtoBotRun): BotRun {
  return {
    id: Number(row.id),
    meetingId: row.meetingId,
    meetingTitle: row.meetingTitle,
    status: row.status as BotRunStatus,
    error: row.error || undefined,
    meetingStartAt: Number(row.meetingStartAt),
    startedAt: row.startedAt ? Number(row.startedAt) : undefined,
    endedAt: row.endedAt ? Number(row.endedAt) : undefined,
    createdAt: Number(row.createdAt),
    recordingPath: row.recordingPath || undefined,
    recordingStorage: row.recordingStorage || undefined,
    durationSecs: row.durationSecs ? Number(row.durationSecs) : undefined,
  }
}

function parseError(error: RecordingError): RecordingApiError {
  return new RecordingApiError(error.code, error.message)
}

function unwrapList(response: ListBotRunsResponse): BotRun[] {
  if (response.result.oneofKind === "success") {
    return response.result.success.runs.map(mapRun)
  }
  if (response.result.oneofKind === "error") {
    throw parseError(response.result.error)
  }
  throw new RecordingApiError("internal_error", "Empty recordings list response.")
}

function unwrapGet(response: GetBotRunResponse): BotRun {
  if (response.result.oneofKind === "success") {
    return mapRun(response.result.success)
  }
  if (response.result.oneofKind === "error") {
    throw parseError(response.result.error)
  }
  throw new RecordingApiError("internal_error", "Empty recording response.")
}

function unwrapPlayback(response: GetPlaybackUrlResponse): { url: string; expiresAt: number } {
  if (response.result.oneofKind === "success") {
    return {
      url: response.result.success.url,
      expiresAt: Number(response.result.success.expiresAt),
    }
  }
  if (response.result.oneofKind === "error") {
    throw parseError(response.result.error)
  }
  throw new RecordingApiError("internal_error", "Empty playback URL response.")
}

function unwrapStartAdHoc(response: StartAdHocRecordingResponse): BotRun {
  if (response.result.oneofKind === "success") {
    const run = response.result.success.run
    if (run) return mapRun(run)
    return mapRun({
      id: response.result.success.runId,
      meetingId: "",
      meetingTitle: "",
      status: "queued",
      error: "",
      meetingStartAt: BigInt(0),
      startedAt: BigInt(0),
      endedAt: BigInt(0),
      createdAt: BigInt(0),
      recordingPath: "",
      recordingStorage: "",
      durationSecs: BigInt(0),
    })
  }
  if (response.result.oneofKind === "error") {
    throw parseError(response.result.error)
  }
  throw new RecordingApiError("internal_error", "Empty ad-hoc recording response.")
}

export async function apiListBotRuns(token: string, limit = 50): Promise<BotRun[]> {
  const client = getRecordingClient()
  const { response } = await client.listBotRuns({ token, limit })
  return unwrapList(response)
}

export async function apiGetBotRun(token: string, runId: number): Promise<BotRun> {
  const client = getRecordingClient()
  const { response } = await client.getBotRun({ token, runId: BigInt(runId) })
  return unwrapGet(response)
}

export async function apiGetPlaybackUrl(
  token: string,
  runId: number,
): Promise<{ url: string; expiresAt: number }> {
  const client = getRecordingClient()
  const { response } = await client.getPlaybackUrl({ token, runId: BigInt(runId) })
  return unwrapPlayback(response)
}

export async function apiStartAdHocRecording(token: string, meetUrl: string): Promise<BotRun> {
  const client = getRecordingClient()
  const { response } = await client.startAdHocRecording({ token, meetUrl })
  return unwrapStartAdHoc(response)
}

export function recordingErrorMessage(error: unknown): string {
  if (error instanceof RecordingApiError) {
    switch (error.code) {
      case "session_not_found":
        return "Your session has expired. Please sign in again."
      case "not_found":
        return "Recording not found."
      case "not_available":
        return "Recording is not available for playback yet."
      case "invalid_meet_url":
        return "Enter a valid Google Meet link (https://meet.google.com/…)."
      case "bot_busy":
        return "The bot is already recording another meeting. Try again when it finishes."
      default:
        return error.message
    }
  }
  if (error instanceof Error) {
    if (error.message.includes("Failed to fetch") || error.message.includes("NetworkError")) {
      return "Cannot reach the server. Is the backend running?"
    }
    return error.message
  }
  return "Could not load recordings."
}
