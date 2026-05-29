import { useState } from "react"
import { Link } from "react-router-dom"
import { ExternalLink, Video } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Switch } from "@/components/ui/switch"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { PageHeader } from "@/components/shared/PageHeader"
import { mockCalendars, mockMeetings } from "@/lib/mock-data"
import { formatDate, formatRelative, meetingTimeRange } from "@/lib/format"
import { toast } from "sonner"

const now = Math.floor(Date.now() / 1000)

export function MeetingsPage() {
  const [meetings, setMeetings] = useState(mockMeetings)

  function toggleAutoRecord(id: string, enabled: boolean) {
    setMeetings((prev) =>
      prev.map((m) => (m.id === id ? { ...m, autoRecord: enabled } : m)),
    )
    toast.success(enabled ? "Auto-record enabled" : "Auto-record disabled")
  }

  const upcoming = meetings.filter((m) => m.startAt > now).sort((a, b) => a.startAt - b.startAt)
  const past = meetings.filter((m) => m.startAt <= now).sort((a, b) => b.startAt - a.startAt)

  function MeetingRow({ meeting }: { meeting: (typeof meetings)[0] }) {
    const calendar = mockCalendars.find((c) => c.id === meeting.calendarId)
    return (
      <Card>
        <CardContent className="flex flex-col gap-4 p-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <Link
                to={`/app/meetings/${meeting.id}`}
                className="font-medium hover:underline"
              >
                {meeting.title}
              </Link>
              {calendar && (
                <Badge variant="outline" className="text-xs">
                  {calendar.label}
                </Badge>
              )}
            </div>
            <p className="text-muted-foreground mt-1 text-sm">
              {formatDate(meeting.startAt)} · {meetingTimeRange(meeting.startAt, meeting.endAt)}
            </p>
            <p className="text-muted-foreground mt-0.5 text-xs">
              {meeting.attendees.length} attendees · {formatRelative(meeting.startAt)}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-4">
            <div className="flex items-center gap-2">
              <Switch
                checked={meeting.autoRecord}
                onCheckedChange={(checked) => toggleAutoRecord(meeting.id, checked)}
              />
              <span className="text-muted-foreground text-xs">Auto-record</span>
            </div>
            <Button variant="outline" size="sm" asChild>
              <a href={meeting.meetUrl} target="_blank" rel="noreferrer">
                <Video className="size-4" />
                Join
              </a>
            </Button>
          </div>
        </CardContent>
      </Card>
    )
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title="Meetings"
        description="Upcoming and past meetings synced from your calendars."
      />

      <Tabs defaultValue="upcoming">
        <TabsList>
          <TabsTrigger value="upcoming">Upcoming ({upcoming.length})</TabsTrigger>
          <TabsTrigger value="past">Past ({past.length})</TabsTrigger>
        </TabsList>
        <TabsContent value="upcoming" className="mt-4 space-y-3">
          {upcoming.length === 0 ? (
            <p className="text-muted-foreground text-sm">No upcoming meetings.</p>
          ) : (
            upcoming.map((m) => <MeetingRow key={m.id} meeting={m} />)
          )}
        </TabsContent>
        <TabsContent value="past" className="mt-4 space-y-3">
          {past.map((m) => <MeetingRow key={m.id} meeting={m} />)}
        </TabsContent>
      </Tabs>
    </div>
  )
}

export function MeetingDetailPage({ meetingId }: { meetingId: string }) {
  const meeting = mockMeetings.find((m) => m.id === meetingId)
  const calendar = meeting ? mockCalendars.find((c) => c.id === meeting.calendarId) : undefined

  if (!meeting) {
    return (
      <div className="space-y-4">
        <PageHeader title="Meeting not found" />
        <Button asChild variant="outline">
          <Link to="/app/meetings">Back to meetings</Link>
        </Button>
      </div>
    )
  }

  const isPast = meeting.startAt <= now

  return (
    <div className="space-y-8">
      <PageHeader
        title={meeting.title}
        description={`${formatDate(meeting.startAt)} · ${meetingTimeRange(meeting.startAt, meeting.endAt)}`}
        action={
          <Button asChild>
            <a href={meeting.meetUrl} target="_blank" rel="noreferrer">
              <ExternalLink className="size-4" />
              Open in Meet
            </a>
          </Button>
        }
      />

      <div className="grid gap-6 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardContent className="space-y-4 p-6">
            {meeting.description && (
              <div>
                <h3 className="text-sm font-medium">Description</h3>
                <p className="text-muted-foreground mt-1 text-sm">{meeting.description}</p>
              </div>
            )}
            <div>
              <h3 className="text-sm font-medium">Attendees</h3>
              <ul className="mt-2 space-y-1">
                {meeting.attendees.map((a) => (
                  <li key={a} className="text-muted-foreground text-sm">{a}</li>
                ))}
              </ul>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="space-y-4 p-6">
            <div>
              <p className="text-muted-foreground text-xs">Calendar</p>
              <p className="text-sm font-medium">{calendar?.label ?? "Unknown"}</p>
            </div>
            <div>
              <p className="text-muted-foreground text-xs">Auto-record</p>
              <Badge variant={meeting.autoRecord ? "default" : "secondary"}>
                {meeting.autoRecord ? "Enabled" : "Disabled"}
              </Badge>
            </div>
            <div>
              <p className="text-muted-foreground text-xs">Status</p>
              <p className="text-sm font-medium">{isPast ? "Completed" : "Upcoming"}</p>
            </div>
            {isPast && (
              <Button variant="outline" className="w-full" asChild>
                <Link to="/app/notes">View notes</Link>
              </Button>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
