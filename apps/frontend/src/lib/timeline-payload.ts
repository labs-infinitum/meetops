import type {
  ActiveSpeakerChangedPayload,
  CollectorWarningPayload,
  MeetingTimelineEventPayload,
  ParticipantJoinedPayload,
  ParticipantLeftPayload,
  ParticipantSnapshotPayload,
  TimelineParticipantRef,
} from "@/types/timeline"

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readString(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key]
  return typeof value === "string" ? value : undefined
}

function readNumber(record: Record<string, unknown>, key: string): number | undefined {
  const value = record[key]
  return typeof value === "number" && Number.isFinite(value) ? value : undefined
}

function parseParticipantRef(value: unknown): TimelineParticipantRef | undefined {
  if (!isRecord(value)) return undefined
  const id = readString(value, "id")
  const name = readString(value, "name")
  const source = readString(value, "source")
  if (!id || !name || !source) return undefined
  return { id, name, source }
}

export function parseTimelinePayloadJson(
  eventType: string,
  payloadJson: string,
): MeetingTimelineEventPayload | undefined {
  if (!payloadJson) return undefined
  let parsed: unknown
  try {
    parsed = JSON.parse(payloadJson)
  } catch {
    return undefined
  }
  if (!isRecord(parsed)) return undefined
  return parseTimelinePayload(eventType, parsed)
}

export function parseTimelinePayload(
  eventType: string,
  parsed: Record<string, unknown>,
): MeetingTimelineEventPayload | undefined {
  switch (eventType) {
    case "participant_joined": {
      const source = readString(parsed, "source")
      if (!source) return undefined
      const payload: ParticipantJoinedPayload = { source }
      const rawText = readString(parsed, "rawText")
      const ariaLabel = readString(parsed, "ariaLabel")
      const scrollTarget = parsed.scrollTarget
      if (rawText) payload.rawText = rawText
      if (ariaLabel) payload.ariaLabel = ariaLabel
      if (scrollTarget === null) {
        payload.scrollTarget = null
      } else {
        const target = readString(parsed, "scrollTarget")
        if (target) payload.scrollTarget = target
      }
      return payload
    }
    case "participant_left": {
      const source = readString(parsed, "source")
      if (!source) return undefined
      const payload: ParticipantLeftPayload = { source }
      return payload
    }
    case "active_speaker_changed": {
      const source = readString(parsed, "source")
      if (!source) return undefined
      const payload: ActiveSpeakerChangedPayload = { source }
      return payload
    }
    case "participant_snapshot": {
      const count = readNumber(parsed, "count")
      if (count === undefined) return undefined
      const payload: ParticipantSnapshotPayload = { count }
      const participants = parsed.participants
      if (Array.isArray(participants)) {
        const mapped = participants
          .map(parseParticipantRef)
          .filter((item): item is TimelineParticipantRef => item !== undefined)
        if (mapped.length > 0) {
          payload.participants = mapped
        }
      }
      const activeSpeakerId = parsed.activeSpeakerId
      if (activeSpeakerId === null) {
        payload.activeSpeakerId = null
      } else {
        const id = readString(parsed, "activeSpeakerId")
        if (id) payload.activeSpeakerId = id
      }
      return payload
    }
    case "collector_warning": {
      const message = readString(parsed, "message")
      if (!message) return undefined
      const payload: CollectorWarningPayload = { message }
      return payload
    }
    default:
      return undefined
  }
}

export function snapshotParticipantCount(
  payload: MeetingTimelineEventPayload | undefined,
): number | undefined {
  if (payload && "count" in payload && typeof payload.count === "number") {
    return payload.count
  }
  return undefined
}
