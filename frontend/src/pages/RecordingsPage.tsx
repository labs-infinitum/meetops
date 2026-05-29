import { Link } from "react-router-dom"
import { AlertCircle, ChevronRight, Film } from "lucide-react"
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
import { mockBotRuns } from "@/lib/mock-data"
import { formatDate, formatDateTime, formatDuration } from "@/lib/format"

export function RecordingsPage() {
  const runs = [...mockBotRuns].sort((a, b) => b.createdAt - a.createdAt)

  return (
    <div className="space-y-8">
      <PageHeader
        title="Recordings"
        description="Bot recording activity for your meetings."
      />

      {runs.length === 0 ? (
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
  const run = mockBotRuns.find((r) => r.id === Number(recordingId))
  const noteLink = run
    ? `/app/notes?meeting=${run.meetingId}`
    : "/app/notes"

  if (!run) {
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
            <div className="bg-muted flex aspect-video items-center justify-center rounded-lg border">
              {run.status === "completed" || run.status === "recording" ? (
                <div className="text-center">
                  <Film className="text-muted-foreground mx-auto size-12" />
                  <p className="text-muted-foreground mt-3 text-sm">
                    Video preview placeholder
                  </p>
                  {run.recordingPath && (
                    <p className="text-muted-foreground mt-1 font-mono text-xs">
                      {run.recordingPath}
                    </p>
                  )}
                </div>
              ) : (
                <div className="text-center">
                  <AlertCircle className="text-muted-foreground mx-auto size-12" />
                  <p className="text-muted-foreground mt-3 text-sm">No recording available</p>
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
            {run.durationSecs && (
              <div>
                <p className="text-muted-foreground text-xs">Duration</p>
                <p className="text-sm">{formatDuration(run.durationSecs)}</p>
              </div>
            )}
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
