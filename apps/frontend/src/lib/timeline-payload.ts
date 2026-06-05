import {
  type ActiveSpeakerChangedPayload,
  type CollectorWarningPayload,
  type MeetingTimelineEventPayload,
  type MeetingTimelineEventType,
  type ParticipantJoinedPayload,
  type ParticipantLeftPayload,
  type ParticipantSnapshotPayload,
  type ScreenShareStartedPayload,
  type ScreenShareStoppedPayload,
  type TimelineParticipantRef,
} from "@/types"
import { isRecord, readNumber, readString } from "@/lib/type-guards"

function parseParticipantRef(value: unknown): TimelineParticipantRef | undefined {
  if (!isRecord(value)) return undefined
  const id = readString(value, "id")
  const name = readString(value, "name")
  const source = readString(value, "source")
  if (!id || !name || !source) return undefined
  return { id, name, source }
}

export function parseTimelinePayloadJson(
  eventType: MeetingTimelineEventType,
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

function parseTimelinePayload(
  eventType: MeetingTimelineEventType,
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
    case "screen_share_started": {
      const source = readString(parsed, "source")
      const presentationId = readString(parsed, "presentationId")
      const presenterName = readString(parsed, "presenterName")
      if (!source || !presentationId || !presenterName) return undefined
      const presenterParticipantId = parsed.presenterParticipantId
      const payload: ScreenShareStartedPayload = {
        source,
        presentationId,
        presenterParticipantId:
          presenterParticipantId === null
            ? null
            : readString(parsed, "presenterParticipantId") ?? null,
        presenterName,
      }
      const presenterMatch = readString(parsed, "presenterMatch")
      if (presenterMatch) payload.presenterMatch = presenterMatch
      const candidateIds = parsed.presenterCandidateIds
      if (Array.isArray(candidateIds)) {
        payload.presenterCandidateIds = candidateIds.filter(
          (id): id is string => typeof id === "string",
        )
      }
      const scrollTarget = parsed.scrollTarget
      if (scrollTarget === null) {
        payload.scrollTarget = null
      } else {
        const target = readString(parsed, "scrollTarget")
        if (target) payload.scrollTarget = target
      }
      return payload
    }
    case "screen_share_stopped": {
      const source = readString(parsed, "source")
      const presentationId = readString(parsed, "presentationId")
      const presenterName = readString(parsed, "presenterName")
      if (!source || !presentationId || !presenterName) return undefined
      const presenterParticipantId = parsed.presenterParticipantId
      const payload: ScreenShareStoppedPayload = {
        source,
        presentationId,
        presenterParticipantId:
          presenterParticipantId === null
            ? null
            : readString(parsed, "presenterParticipantId") ?? null,
        presenterName,
      }
      const presenterMatch = readString(parsed, "presenterMatch")
      if (presenterMatch) payload.presenterMatch = presenterMatch
      const candidateIds = parsed.presenterCandidateIds
      if (Array.isArray(candidateIds)) {
        payload.presenterCandidateIds = candidateIds.filter(
          (id): id is string => typeof id === "string",
        )
      }
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
