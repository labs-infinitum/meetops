import { useEffect, useState } from "react"
import { Link } from "react-router-dom"
import {
  Calendar,
  CheckSquare,
  ChevronRight,
  Loader2,
  Video,
  CalendarDays,
} from "lucide-react"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { PageHeader } from "@/components/shared/PageHeader"
import { StatCard } from "@/components/shared/StatCard"
import {
  apiGetDashboardSummary,
  meetingErrorMessage,
  meetingPath,
} from "@/lib/meeting-api"
import {
  apiListBotRuns,
  apiListGeneratedNotes,
  recordingErrorMessage,
} from "@/lib/recording-api"
import { formatDate, formatRelative, meetingTimeRange } from "@/lib/format"
import { useAuth } from "@/lib/auth-context"
import { StatusBadge } from "@/components/shared/StatusBadge"
import type { BotRun, Meeting } from "@/types"
import { toast } from "sonner"

export function DashboardPage() {
  const { session } = useAuth()
  const [loading, setLoading] = useState(true)
  const [upcomingMeetings, setUpcomingMeetings] = useState<Meeting[]>([])
  const [stats, setStats] = useState({
    upcoming7d: 0,
    connectedCalendars: 0,
    completedRecordings: 0,
    openTodos: 0,
  })

  const [recentRuns, setRecentRuns] = useState<BotRun[]>([])

  useEffect(() => {
    if (!session?.token) return
    setLoading(true)
    Promise.all([
      apiGetDashboardSummary(session.token),
      apiListBotRuns(session.token, 4),
      apiListGeneratedNotes(session.token, 100),
    ])
      .then(([summary, runs, notes]) => {
        setUpcomingMeetings(summary.upcomingMeetings)
        setRecentRuns(runs)
        setStats({
          upcoming7d: summary.upcoming7d,
          connectedCalendars: summary.connectedCalendars,
          completedRecordings: summary.completedRecordings,
          openTodos: notes.reduce((sum, note) => sum + note.actionItems.length, 0),
        })
      })
      .catch((error) => toast.error(meetingErrorMessage(error) || recordingErrorMessage(error)))
      .finally(() => setLoading(false))
  }, [session?.token])

  return (
    <div className="space-y-8">
      <PageHeader
        title="Dashboard"
        description="Overview of your meetings, recordings, and notes."
        action={
          <Button asChild>
            <Link to="/app/calendars">Connect calendar</Link>
          </Button>
        }
      />

      {loading && (
        <div className="text-muted-foreground flex items-center gap-2 text-sm">
          <Loader2 className="size-4 animate-spin" />
          Loading dashboard…
        </div>
      )}

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard title="Upcoming meetings" value={stats.upcoming7d} icon={CalendarDays} subtitle="Next 7 days" />
        <StatCard title="Recordings" value={stats.completedRecordings} icon={Video} subtitle="Completed" />
        <StatCard title="Open todos" value={stats.openTodos} icon={CheckSquare} subtitle="From recent meetings" />
        <StatCard title="Calendars" value={stats.connectedCalendars} icon={Calendar} subtitle="Connected accounts" />
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <div>
              <CardTitle className="text-base">Upcoming meetings</CardTitle>
              <CardDescription>Synced from your connected calendars</CardDescription>
            </div>
            <Button variant="ghost" size="sm" asChild>
              <Link to="/app/meetings">
                View all
                <ChevronRight className="size-4" />
              </Link>
            </Button>
          </CardHeader>
          <CardContent className="space-y-3">
            {upcomingMeetings.length === 0 ? (
              <p className="text-muted-foreground text-sm">No upcoming meetings.</p>
            ) : (
              upcomingMeetings.map((meeting) => (
                <Link
                  key={meeting.id}
                  to={meetingPath(meeting.id)}
                  className="hover:bg-muted/50 flex items-center justify-between rounded-lg border p-3 transition-colors"
                >
                  <div className="min-w-0">
                    <p className="truncate text-sm font-medium">{meeting.title}</p>
                    <p className="text-muted-foreground text-xs">
                      {formatDate(meeting.startAt)} · {meetingTimeRange(meeting.startAt, meeting.endAt)}
                    </p>
                  </div>
                  <div className="ml-3 flex shrink-0 items-center gap-2">
                    {meeting.autoRecord && (
                      <Badge variant="secondary" className="text-xs">Auto</Badge>
                    )}
                    <span className="text-muted-foreground text-xs">{formatRelative(meeting.startAt)}</span>
                  </div>
                </Link>
              ))
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <div>
              <CardTitle className="text-base">Recording activity</CardTitle>
              <CardDescription>Recent bot runs</CardDescription>
            </div>
            <Button variant="ghost" size="sm" asChild>
              <Link to="/app/recordings">
                View all
                <ChevronRight className="size-4" />
              </Link>
            </Button>
          </CardHeader>
          <CardContent className="space-y-3">
            {recentRuns.length === 0 ? (
              <p className="text-muted-foreground text-sm">No recording activity yet.</p>
            ) : (
              recentRuns.map((run) => (
                <Link
                  key={run.id}
                  to={`/app/recordings/${run.id}`}
                  className="hover:bg-muted/50 flex items-center justify-between rounded-lg border p-3 transition-colors"
                >
                  <div className="min-w-0">
                    <p className="truncate text-sm font-medium">{run.meetingTitle}</p>
                    <p className="text-muted-foreground text-xs">{formatDate(run.meetingStartAt)}</p>
                  </div>
                  <StatusBadge status={run.status} />
                </Link>
              ))
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
