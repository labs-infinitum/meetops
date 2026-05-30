import { useEffect, useState } from "react"
import { Link } from "react-router-dom"
import { AlertCircle, ChevronRight, Film, Loader2 } from "lucide-react"
import { Button } from "@/components/ui/button"
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
  RecordingApiError,
  recordingErrorMessage,
} from "@/lib/recording-api"
import { formatDate, formatDateTime, formatDuration } from "@/lib/format"
import { useAuth } from "@/lib/auth-context"
import type { BotRun } from "@/types"
import { toast } from "sonner"

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
  const [loading, setLoading] = useState(true)
  const [notFound, setNotFound] = useState(false)

  const runId = Number(recordingId)
  const noteLink = run ? `/app/notes?meeting=${encodeURIComponent(run.meetingId)}` : "/app/notes"

  useEffect(() => {
    if (!session?.token || Number.isNaN(runId)) return
    setLoading(true)
    setNotFound(false)
    setPlaybackUrl(null)

    apiGetBotRun(session.token, runId)
      .then(async (loaded) => {
        setRun(loaded)
        if (
          loaded.status === "completed" &&
          loaded.recordingPath &&
          loaded.recordingStorage === "s3"
        ) {
          const playback = await apiGetPlaybackUrl(session.token, runId)
          setPlaybackUrl(playback.url)
        }
      })
      .catch((error) => {
        if (error instanceof RecordingApiError && error.code === "not_found") {
          setNotFound(true)
        } else {
          toast.error(recordingErrorMessage(error))
        }
      })
      .finally(() => setLoading(false))
  }, [session?.token, runId])

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
          run.status === "completed" ? (
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
              ) : run.status === "recording" || run.status === "completed" ? (
                <div className="flex aspect-video items-center justify-center">
                  <div className="text-center">
                    <Film className="text-muted-foreground mx-auto size-12" />
                    <p className="text-muted-foreground mt-3 text-sm">
                      {run.status === "recording"
                        ? "Recording in progress…"
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
            {run.recordingStorage && (
              <div>
                <p className="text-muted-foreground text-xs">Storage</p>
                <p className="text-sm font-medium uppercase">{run.recordingStorage}</p>
              </div>
            )}
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
    </div>
  )
}
