import { useEffect, useState } from "react"
import { Link } from "react-router-dom"
import { AlertCircle, ChevronRight, Film, Loader2, LogIn, LogOut, Mic2, Square, Users } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent } from "@/components/ui/card"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { PageHeader } from "@/components/shared/PageHeader"
import { StatusBadge } from "@/components/shared/StatusBadge"
import { EmptyState } from "@/components/shared/EmptyState"
import {
  apiGetBotRun,
  apiGetPlaybackUrl,
  apiListBotRuns,
  apiListMeetingTimelineEvents,
  apiStopRecording,
  isActiveBotRun,
  RecordingApiError,
  recordingErrorMessage,
} from "@/lib/recording-api"
import { formatDate, formatDateTime, formatDuration } from "@/lib/format"
import { useAuth } from "@/lib/auth-context"
import { snapshotParticipantCount } from "@/lib/timeline-payload"
import type { BotRun, MeetingTimelineEvent } from "@/types"
import { toast } from "sonner"

function formatRelativeTime(relativeMs: number): string {
  return formatDuration(Math.max(0, Math.floor(relativeMs / 1000)))
}

function eventTitle(event: MeetingTimelineEvent): string {
  switch (event.eventType) {
    case "participant_joined":
      return `${event.participantName || "Someone"} joined`
    case "participant_left":
      return `${event.participantName || "Someone"} left`
    case "active_speaker_changed":
      return `${event.participantName || "Unknown speaker"} started speaking`
    case "participant_snapshot": {
      const count = snapshotParticipantCount(event.payload)
      return count === undefined ? "Participant snapshot" : `${count} participants in meeting`
    }
    default:
      return event.eventType.replaceAll("_", " ")
  }
}

function eventIcon(eventType: string) {
  switch (eventType) {
    case "participant_joined":
      return LogIn
    case "participant_left":
      return LogOut
    case "active_speaker_changed":
      return Mic2
    default:
      return Users
  }
}

function timelineEventsToShow(events: MeetingTimelineEvent[]): MeetingTimelineEvent[] {
  const withoutFrequentSnapshots = events.filter((event) => event.eventType !== "participant_snapshot")
  return (withoutFrequentSnapshots.length > 0 ? withoutFrequentSnapshots : events).slice(0, 100)
}

export function RecordingsPage() {
  const { session } = useAuth()
  const [runs, setRuns] = useState<BotRun[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (!session?.token) return
    setLoading(true)
    apiListBotRuns(session.token)
      .then(setRuns)
      .catch((error) => toast.error(recordingErrorMessage(error)))
      .finally(() => setLoading(false))
  }, [session?.token])

  return (
    <div className="space-y-8">
      <PageHeader
        title="Recordings"
        description="Bot recording activity for your meetings."
      />

      {loading && (
        <div className="text-muted-foreground flex items-center gap-2 text-sm">
          <Loader2 className="size-4 animate-spin" />
          Loading recordings…
        </div>
      )}

      {!loading && runs.length === 0 ? (
        <EmptyState
          icon={Film}
          title="No recordings yet"
          description="Recordings appear here when the bot joins and records your meetings."
          action={
            <Button asChild>
              <Link to="/app/meetings">View meetings</Link>
            </Button>
          }
        />
      ) : (
        <Card>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Meeting</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className="hidden md:table-cell">Started</TableHead>
                  <TableHead className="hidden sm:table-cell">Duration</TableHead>
                  <TableHead className="w-10" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {runs.map((run) => (
                  <TableRow key={run.id}>
                    <TableCell>
                      <div>
                        <p className="font-medium">{run.meetingTitle}</p>
                        <p className="text-muted-foreground text-xs">
                          {formatDate(run.meetingStartAt)}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={run.status} />
                    </TableCell>
                    <TableCell className="text-muted-foreground hidden text-sm md:table-cell">
                      {run.startedAt ? formatDateTime(run.startedAt) : "—"}
                    </TableCell>
                    <TableCell className="text-muted-foreground hidden text-sm sm:table-cell">
                      {run.durationSecs ? formatDuration(run.durationSecs) : "—"}
                    </TableCell>
                    <TableCell>
                      <Button variant="ghost" size="icon" asChild>
                        <Link to={`/app/recordings/${run.id}`}>
                          <ChevronRight className="size-4" />
                        </Link>
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  )
}

export function RecordingDetailPage({ recordingId }: { recordingId: string }) {
  const { session } = useAuth()
  const [run, setRun] = useState<BotRun | null>(null)
  const [playbackUrl, setPlaybackUrl] = useState<string | null>(null)
  const [timelineEvents, setTimelineEvents] = useState<MeetingTimelineEvent[]>([])
  const [loading, setLoading] = useState(true)
  const [notFound, setNotFound] = useState(false)
  const [stopping, setStopping] = useState(false)

  const runId = Number(recordingId)
  const noteLink = run ? `/app/notes?meeting=${encodeURIComponent(run.meetingId)}` : "/app/notes"
  const isActive = run ? isActiveBotRun(run.status) : false

  async function loadRun(showLoading = false) {
    if (!session?.token || Number.isNaN(runId)) return
    if (showLoading) setLoading(true)

    try {
      const loaded = await apiGetBotRun(session.token, runId)
      setRun(loaded)
      setNotFound(false)
      setTimelineEvents(await apiListMeetingTimelineEvents(session.token, runId))

      if (
        loaded.status === "completed" &&
        loaded.recordingPath
      ) {
        const playback = await apiGetPlaybackUrl(session.token, runId)
        setPlaybackUrl(playback.url)
      } else if (!isActiveBotRun(loaded.status)) {
        setPlaybackUrl(null)
      }
    } catch (error) {
      if (error instanceof RecordingApiError && error.code === "not_found") {
        setNotFound(true)
      } else {
        toast.error(recordingErrorMessage(error))
      }
    } finally {
      if (showLoading) setLoading(false)
    }
  }

  useEffect(() => {
    loadRun(true)
  }, [session?.token, runId])

  useEffect(() => {
    if (!session?.token || Number.isNaN(runId) || !isActive) return
    const interval = window.setInterval(() => {
      loadRun()
    }, 3000)
    return () => window.clearInterval(interval)
  }, [session?.token, runId, isActive])

  async function stopRecording() {
    if (!session?.token || !run) return
    setStopping(true)
    try {
      await apiStopRecording(session.token, run.id)
      toast.success("Stopping bot and ending recording…")
      await loadRun()
    } catch (error) {
      toast.error(recordingErrorMessage(error))
    } finally {
      setStopping(false)
    }
  }

  if (loading) {
    return (
      <div className="text-muted-foreground flex items-center gap-2 text-sm">
        <Loader2 className="size-4 animate-spin" />
        Loading recording…
      </div>
    )
  }

  if (notFound || !run) {
    return (
      <div className="space-y-4">
        <PageHeader title="Recording not found" />
        <Button asChild variant="outline">
          <Link to="/app/recordings">Back to recordings</Link>
        </Button>
      </div>
    )
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title={run.meetingTitle}
        description={`Run #${run.id} · ${formatDate(run.meetingStartAt)}`}
        action={
          isActive ? (
            <Button variant="destructive" onClick={stopRecording} disabled={stopping}>
              {stopping ? <Loader2 className="animate-spin" /> : <Square className="size-4" />}
              Stop bot
            </Button>
          ) : run.status === "completed" ? (
            <Button asChild>
              <Link to={noteLink}>View AI notes</Link>
            </Button>
          ) : undefined
        }
      />

      <div className="grid gap-6 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardContent className="p-6">
            <div className="bg-muted overflow-hidden rounded-lg border">
              {playbackUrl ? (
                <video
                  src={playbackUrl}
                  controls
                  className="aspect-video w-full bg-black"
                  preload="metadata"
                >
                  Your browser does not support video playback.
                </video>
              ) : isActive || run.status === "completed" ? (
                <div className="flex aspect-video items-center justify-center">
                  <div className="text-center">
                    <Film className="text-muted-foreground mx-auto size-12" />
                    <p className="text-muted-foreground mt-3 text-sm">
                      {run.status === "recording"
                        ? "Recording in progress…"
                        : run.status === "joining" || run.status === "in_meeting"
                          ? "Bot is joining the meeting…"
                          : isActive
                            ? "Bot is starting…"
                            : "Video is not available yet."}
                    </p>
                  </div>
                </div>
              ) : (
                <div className="flex aspect-video items-center justify-center">
                  <div className="text-center">
                    <AlertCircle className="text-muted-foreground mx-auto size-12" />
                    <p className="text-muted-foreground mt-3 text-sm">No recording available</p>
                  </div>
                </div>
              )}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="space-y-4 p-6">
            <div>
              <p className="text-muted-foreground text-xs">Status</p>
              <div className="mt-1">
                <StatusBadge status={run.status} />
              </div>
            </div>
            {run.startedAt && (
              <div>
                <p className="text-muted-foreground text-xs">Started</p>
                <p className="text-sm">{formatDateTime(run.startedAt)}</p>
              </div>
            )}
            {run.endedAt && (
              <div>
                <p className="text-muted-foreground text-xs">Ended</p>
                <p className="text-sm">{formatDateTime(run.endedAt)}</p>
              </div>
            )}
            {run.durationSecs ? (
              <div>
                <p className="text-muted-foreground text-xs">Duration</p>
                <p className="text-sm">{formatDuration(run.durationSecs)}</p>
              </div>
            ) : null}
            {run.error && (
              <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-3">
                <p className="text-destructive text-xs font-medium">Error</p>
                <p className="text-destructive/80 mt-1 text-sm">{run.error}</p>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardContent className="p-6">
          <div className="flex items-center justify-between gap-4">
            <div>
              <h2 className="text-base font-semibold">Meeting activity</h2>
              <p className="text-muted-foreground text-sm">
                Participant changes and detected active speaker updates from the Meet UI.
              </p>
            </div>
            <Badge variant="secondary">{timelineEvents.length} events</Badge>
          </div>

          {timelineEvents.length === 0 ? (
            <p className="text-muted-foreground mt-6 text-sm">
              No meeting activity has been captured for this recording yet.
            </p>
          ) : (
            <div className="mt-6 space-y-4">
              {timelineEventsToShow(timelineEvents).map((event) => {
                const Icon = eventIcon(event.eventType)
                return (
                  <div key={event.id} className="flex gap-3">
                    <div className="bg-muted mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-full">
                      <Icon className="text-muted-foreground size-4" />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-sm font-medium">{eventTitle(event)}</p>
                        <Badge variant="outline">{formatRelativeTime(event.relativeMs)}</Badge>
                      </div>
                      <p className="text-muted-foreground mt-1 text-xs">
                        {formatDateTime(event.occurredAt)}
                        {event.participantId ? ` · ${event.participantId}` : ""}
                      </p>
                    </div>
                  </div>
                )
              })}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
