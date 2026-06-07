import { useEffect, useState } from "react"
import { Link } from "react-router-dom"
import {
  AlertCircle,
  ChevronRight,
  FileText,
  Film,
  ListChecks,
  Loader2,
  LogIn,
  LogOut,
  Mic2,
  MonitorOff,
  MonitorUp,
  Sparkles,
  Square,
  Users,
} from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Separator } from "@/components/ui/separator"
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
  apiListGeneratedNotes,
  apiListMeetingTimelineEvents,
  apiStopRecording,
  RecordingApiError,
  recordingErrorMessage,
} from "@/lib/recording-api"
import { isActiveBotRun } from "@/types"
import { formatDate, formatDateTime, formatDuration } from "@/lib/format"
import { useAuth } from "@/lib/auth-context"
import { snapshotParticipantCount } from "@/lib/timeline-payload"
import { cn } from "@/lib/utils"
import type { BotRun, GeneratedNote, MeetingTimelineEvent, MeetingTimelineEventType } from "@/types"
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
    case "screen_share_started":
      return `${event.participantName || "Someone"} started presenting`
    case "screen_share_stopped":
      return `${event.participantName || "Someone"} stopped presenting`
    case "active_speaker_changed":
      return `${event.participantName || "Unknown speaker"} started speaking`
    case "participant_snapshot": {
      const count = snapshotParticipantCount(event.payload)
      return count === undefined ? "Participant snapshot" : `${count} participants in meeting`
    }
    case "collector_warning":
      return "Collector warning"
  }
}

function eventIcon(eventType: MeetingTimelineEventType) {
  switch (eventType) {
    case "participant_joined":
      return LogIn
    case "participant_left":
      return LogOut
    case "screen_share_started":
      return MonitorUp
    case "screen_share_stopped":
      return MonitorOff
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

function latestActiveSpeakerEvent(events: MeetingTimelineEvent[]): MeetingTimelineEvent | undefined {
  return events
    .filter((event) => event.eventType === "active_speaker_changed" && event.participantName)
    .at(-1)
}

function aiStatusVariant(status?: string): "default" | "secondary" | "destructive" | "outline" {
  switch (status) {
    case "completed":
    case "cleaned":
      return "default"
    case "failed":
      return "destructive"
    case "processing":
    case "transcribed":
      return "secondary"
    default:
      return "outline"
  }
}

function renderMarkdownLite(markdown: string) {
  return markdown.split("\n").map((line, i) => {
    if (line.startsWith("## ")) {
      return (
        <h2 key={i} className="mt-6 mb-2 text-lg font-semibold first:mt-0">
          {line.slice(3)}
        </h2>
      )
    }
    if (line.startsWith("### ")) {
      return (
        <h3 key={i} className="mt-4 mb-1 text-base font-medium">
          {line.slice(4)}
        </h3>
      )
    }
    if (line.startsWith("- ")) {
      return (
        <p key={i} className="text-muted-foreground ml-4 text-sm">
          • {line.slice(2)}
        </p>
      )
    }
    if (/^\d+\./.test(line)) {
      return (
        <p key={i} className="text-muted-foreground ml-4 text-sm">
          {line}
        </p>
      )
    }
    if (line.trim() === "") return <br key={i} />
    return (
      <p key={i} className="text-muted-foreground text-sm leading-relaxed">
        {line}
      </p>
    )
  })
}

async function fetchRecordingDetail(token: string, runId: number) {
  const loaded = await apiGetBotRun(token, runId)
  const [timelineEvents, generatedNotes] = await Promise.all([
    apiListMeetingTimelineEvents(token, runId),
    apiListGeneratedNotes(token, 100).catch(() => []),
  ])
  const generatedNote = generatedNotes.find((note) => note.botRunId === runId) ?? null
  let playbackUrl: string | null = null
  if (loaded.status === "completed" && loaded.recordingPath) {
    const playback = await apiGetPlaybackUrl(token, runId)
    playbackUrl = playback.url
  }
  return { loaded, timelineEvents, generatedNote, playbackUrl }
}

export function RecordingsPage() {
  const { session } = useAuth()
  const token = session?.token
  const [runs, setRuns] = useState<BotRun[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (!token) return
    apiListBotRuns(token)
      .then(setRuns)
      .catch((error) => toast.error(recordingErrorMessage(error)))
      .finally(() => setLoading(false))
  }, [token])

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
  const token = session?.token
  const [run, setRun] = useState<BotRun | null>(null)
  const [playbackUrl, setPlaybackUrl] = useState<string | null>(null)
  const [timelineEvents, setTimelineEvents] = useState<MeetingTimelineEvent[]>([])
  const [generatedNote, setGeneratedNote] = useState<GeneratedNote | null>(null)
  const [loading, setLoading] = useState(true)
  const [notFound, setNotFound] = useState(false)
  const [stopping, setStopping] = useState(false)

  const runId = Number(recordingId)
  const noteLink = generatedNote ? `/app/notes/${generatedNote.transcriptId}` : "/app/notes"
  const isActive = run ? isActiveBotRun(run.status) : false
  const activeSpeakerEvent = latestActiveSpeakerEvent(timelineEvents)

  useEffect(() => {
    if (!token || Number.isNaN(runId)) return
    void Promise.resolve()
      .then(() => fetchRecordingDetail(token, runId))
      .then(({ loaded, timelineEvents, generatedNote, playbackUrl }) => {
        setRun(loaded)
        setNotFound(false)
        setTimelineEvents(timelineEvents)
        setGeneratedNote(generatedNote)
        setPlaybackUrl(playbackUrl)
      })
      .catch((error) => {
        if (error instanceof RecordingApiError && error.code === "not_found") {
          setNotFound(true)
        } else {
          toast.error(recordingErrorMessage(error))
        }
      })
      .finally(() => setLoading(false))
  }, [token, runId])

  useEffect(() => {
    if (!token || Number.isNaN(runId) || !isActive) return
    const interval = window.setInterval(() => {
      void fetchRecordingDetail(token, runId)
        .then(({ loaded, timelineEvents, generatedNote, playbackUrl }) => {
          setRun(loaded)
          setTimelineEvents(timelineEvents)
          setGeneratedNote(generatedNote)
          setPlaybackUrl(playbackUrl)
        })
        .catch((error) => toast.error(recordingErrorMessage(error)))
    }, 3000)
    return () => window.clearInterval(interval)
  }, [token, runId, isActive])

  async function stopRecording() {
    if (!token || !run) return
    setStopping(true)
    try {
      await apiStopRecording(token, run.id)
      toast.success("Stopping bot and ending recording…")
      const detail = await fetchRecordingDetail(token, runId)
      setRun(detail.loaded)
      setTimelineEvents(detail.timelineEvents)
      setGeneratedNote(detail.generatedNote)
      setPlaybackUrl(detail.playbackUrl)
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
        <CardHeader>
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <CardTitle className="flex items-center gap-2 text-base">
                <FileText className="size-4" />
                Transcript and AI outputs
              </CardTitle>
              <CardDescription>
                Transcription, speaker-attributed transcript, notes, and action items for this recording.
              </CardDescription>
            </div>
            {generatedNote ? (
              <div className="flex flex-wrap gap-1.5">
                <Badge variant={aiStatusVariant(generatedNote.transcriptStatus)}>
                  Transcript {generatedNote.transcriptStatus}
                </Badge>
                <Badge variant={aiStatusVariant(generatedNote.summaryStatus)}>
                  Notes {generatedNote.summaryStatus || "pending"}
                </Badge>
              </div>
            ) : (
              <Badge variant="outline">Awaiting AI pipeline</Badge>
            )}
          </div>
        </CardHeader>
        <CardContent className="space-y-6">
          {!generatedNote ? (
            <p className="text-muted-foreground text-sm">
              {run.status === "completed"
                ? "The recording is complete, but transcript data has not been created yet."
                : "Transcript data will appear here once the recording completes and the AI pipeline starts."}
            </p>
          ) : (
            <>
              {generatedNote.error && (
                <div className="border-destructive/30 bg-destructive/5 flex gap-3 rounded-lg border p-3">
                  <AlertCircle className="text-destructive mt-0.5 size-4 shrink-0" />
                  <p className="text-destructive text-sm">{generatedNote.error}</p>
                </div>
              )}

              <div className="grid gap-4 lg:grid-cols-3">
                <div className="lg:col-span-2">
                  <div className="mb-2 flex items-center gap-2">
                    <Sparkles className="text-primary size-4" />
                    <h3 className="text-sm font-semibold">TL;DR</h3>
                  </div>
                  <p className="text-muted-foreground text-sm leading-relaxed">
                    {generatedNote.tldr || "TL;DR will appear after notes are generated."}
                  </p>
                </div>

                <div>
                  <div className="mb-2 flex items-center gap-2">
                    <ListChecks className="text-primary size-4" />
                    <h3 className="text-sm font-semibold">Action items</h3>
                  </div>
                  {generatedNote.actionItems.length === 0 ? (
                    <p className="text-muted-foreground text-sm">No action items generated yet.</p>
                  ) : (
                    <div className="space-y-2">
                      {generatedNote.actionItems.slice(0, 4).map((item, index) => (
                        <div key={`${item.text}-${index}`} className="rounded-md border p-2">
                          <p className="text-sm">{item.text}</p>
                          <div className="mt-1 flex flex-wrap gap-1.5">
                            {item.owner && <Badge variant="outline">{item.owner}</Badge>}
                            {item.due && <Badge variant="secondary">Due {item.due}</Badge>}
                          </div>
                        </div>
                      ))}
                      {generatedNote.actionItems.length > 4 && (
                        <p className="text-muted-foreground text-xs">
                          +{generatedNote.actionItems.length - 4} more in the Notes page
                        </p>
                      )}
                    </div>
                  )}
                </div>
              </div>

              {generatedNote.summaryMd && (
                <>
                  <Separator />
                  <div>
                    <h3 className="mb-3 text-sm font-semibold">Meeting notes</h3>
                    <div className="prose prose-sm dark:prose-invert max-w-none">
                      {renderMarkdownLite(generatedNote.summaryMd)}
                    </div>
                  </div>
                </>
              )}

              <Separator />

              <div className="grid gap-4 lg:grid-cols-2">
                <div>
                  <div className="mb-2 flex items-center gap-2">
                    <Mic2 className="text-primary size-4" />
                    <h3 className="text-sm font-semibold">Speaker-attributed transcript</h3>
                  </div>
                  {generatedNote.cleanedTranscript ? (
                    <pre className="bg-muted text-muted-foreground max-h-96 overflow-auto rounded-lg border p-4 whitespace-pre-wrap text-sm leading-relaxed">
                      {generatedNote.cleanedTranscript}
                    </pre>
                  ) : (
                    <p className="text-muted-foreground text-sm">
                      Cleaned speaker-attributed transcript is not available yet.
                    </p>
                  )}
                </div>

                <div>
                  <h3 className="mb-2 text-sm font-semibold">Raw transcription</h3>
                  {generatedNote.rawTranscript ? (
                    <pre className="bg-muted text-muted-foreground max-h-96 overflow-auto rounded-lg border p-4 whitespace-pre-wrap text-xs leading-relaxed">
                      {generatedNote.rawTranscript}
                    </pre>
                  ) : (
                    <p className="text-muted-foreground text-sm">
                      Raw transcription is not available yet.
                    </p>
                  )}
                </div>
              </div>
            </>
          )}
        </CardContent>
      </Card>

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

          {activeSpeakerEvent && (
            <div className="bg-muted/60 mt-6 flex items-center gap-3 rounded-lg border p-3">
              <div className="bg-background flex size-9 shrink-0 items-center justify-center rounded-full border">
                <Mic2 className="text-primary size-4" />
              </div>
              <div className="min-w-0">
                <p className="text-sm font-medium">
                  Current active speaker: {activeSpeakerEvent.participantName}
                </p>
                <p className="text-muted-foreground text-xs">
                  Updated at {formatRelativeTime(activeSpeakerEvent.relativeMs)}
                </p>
              </div>
            </div>
          )}

          {timelineEvents.length === 0 ? (
            <p className="text-muted-foreground mt-6 text-sm">
              No meeting activity has been captured for this recording yet.
            </p>
          ) : (
            <div className="mt-6 space-y-4">
              {timelineEventsToShow(timelineEvents).map((event) => {
                const Icon = eventIcon(event.eventType)
                const isSpeakerEvent = event.eventType === "active_speaker_changed"
                return (
                  <div key={event.id} className="flex gap-3">
                    <div
                      className={cn(
                        "mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-full",
                        isSpeakerEvent ? "bg-primary/10" : "bg-muted",
                      )}
                    >
                      <Icon
                        className={cn(
                          "size-4",
                          isSpeakerEvent ? "text-primary" : "text-muted-foreground",
                        )}
                      />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-sm font-medium">{eventTitle(event)}</p>
                        {isSpeakerEvent && <Badge variant="secondary">Active speaker</Badge>}
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
