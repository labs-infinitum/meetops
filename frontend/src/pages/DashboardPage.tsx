import { Link } from "react-router-dom"
import {
  Calendar,
  CheckSquare,
  ChevronRight,
  Video,
  CalendarDays,
} from "lucide-react"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { PageHeader } from "@/components/shared/PageHeader"
import { StatCard } from "@/components/shared/StatCard"
import { StatusBadge } from "@/components/shared/StatusBadge"
import {
  getDashboardStats,
  mockBotRuns,
  mockMeetings,
  mockNotes,
} from "@/lib/mock-data"
import { formatDate, formatRelative, meetingTimeRange } from "@/lib/format"

const now = Math.floor(Date.now() / 1000)

export function DashboardPage() {
  const stats = getDashboardStats()
  const upcomingMeetings = mockMeetings
    .filter((m) => m.startAt > now)
    .sort((a, b) => a.startAt - b.startAt)
    .slice(0, 4)
  const recentRuns = mockBotRuns.slice(0, 4)
  const latestNote = mockNotes[0]

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

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard title="Upcoming meetings" value={stats.upcoming} icon={CalendarDays} subtitle="Next 7 days" />
        <StatCard title="Recordings" value={stats.recordings} icon={Video} subtitle="Completed this month" />
        <StatCard title="Open todos" value={stats.openTodos} icon={CheckSquare} subtitle="From recent meetings" />
        <StatCard title="Calendars" value={stats.connectedCalendars} icon={Calendar} subtitle="Connected accounts" />
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <div>
              <CardTitle className="text-base">Upcoming meetings</CardTitle>
              <CardDescription>Auto-record enabled meetings</CardDescription>
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
                  to={`/app/meetings/${meeting.id}`}
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
            {recentRuns.map((run) => (
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
            ))}
          </CardContent>
        </Card>
      </div>

      {latestNote && (
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <div>
              <CardTitle className="text-base">Latest AI summary</CardTitle>
              <CardDescription>{latestNote.meetingTitle}</CardDescription>
            </div>
            <Button variant="ghost" size="sm" asChild>
              <Link to={`/app/notes/${latestNote.id}`}>
                Read full note
                <ChevronRight className="size-4" />
              </Link>
            </Button>
          </CardHeader>
          <CardContent>
            <p className="text-sm leading-relaxed">{latestNote.tldr}</p>
            <div className="mt-3 flex flex-wrap gap-1.5">
              {latestNote.tags.map((tag) => (
                <Badge key={tag} variant="outline" className="text-xs">
                  {tag}
                </Badge>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
