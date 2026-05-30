import { useEffect, useState } from "react"
import { Link } from "react-router-dom"
import { ExternalLink, Loader2, Video } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Switch } from "@/components/ui/switch"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { PageHeader } from "@/components/shared/PageHeader"
import {
  apiGetMeeting,
  apiListMeetings,
  apiSetAutoRecord,
  MeetingApiError,
  meetingErrorMessage,
  meetingPath,
} from "@/lib/meeting-api"
import { formatDate, formatRelative, meetingTimeRange } from "@/lib/format"
import { useAuth } from "@/lib/auth-context"
import type { Meeting } from "@/types"
import { toast } from "sonner"

const now = Math.floor(Date.now() / 1000)

export function MeetingsPage() {
  const { session } = useAuth()
  const [meetings, setMeetings] = useState<Meeting[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (!session?.token) return
    setLoading(true)
    apiListMeetings(session.token)
      .then(setMeetings)
      .catch((error) => toast.error(meetingErrorMessage(error)))
      .finally(() => setLoading(false))
  }, [session?.token])

  async function toggleAutoRecord(id: string, enabled: boolean) {
    if (!session?.token) return
    try {
      await apiSetAutoRecord(session.token, id, enabled)
      setMeetings((prev) =>
        prev.map((m) => (m.id === id ? { ...m, autoRecord: enabled } : m)),
      )
      toast.success(enabled ? "Auto-record enabled" : "Auto-record disabled")
    } catch (error) {
      toast.error(meetingErrorMessage(error))
    }
  }

  const upcoming = meetings.filter((m) => m.startAt > now).sort((a, b) => a.startAt - b.startAt)
  const past = meetings.filter((m) => m.startAt <= now).sort((a, b) => b.startAt - a.startAt)

  function MeetingRow({ meeting }: { meeting: Meeting }) {
    return (
      <Card>
        <CardContent className="flex flex-col gap-4 p-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <Link
                to={meetingPath(meeting.id)}
                className="font-medium hover:underline"
              >
                {meeting.title}
              </Link>
              {meeting.calendarLabel && (
                <Badge variant="outline" className="text-xs">
                  {meeting.calendarLabel}
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

      {loading && (
        <div className="text-muted-foreground flex items-center gap-2 text-sm">
          <Loader2 className="size-4 animate-spin" />
          Loading meetings…
        </div>
      )}

      {!loading && meetings.length === 0 && (
        <p className="text-muted-foreground text-sm">
          No meetings synced yet. Connect a calendar and sync to see Google Meet events here.
        </p>
      )}

      {!loading && meetings.length > 0 && (
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
            {past.length === 0 ? (
              <p className="text-muted-foreground text-sm">No past meetings.</p>
            ) : (
              past.map((m) => <MeetingRow key={m.id} meeting={m} />)
            )}
          </TabsContent>
        </Tabs>
      )}
    </div>
  )
}

export function MeetingDetailPage({ meetingId }: { meetingId: string }) {
  const { session } = useAuth()
  const [meeting, setMeeting] = useState<Meeting | null>(null)
  const [loading, setLoading] = useState(true)
  const [notFound, setNotFound] = useState(false)

  useEffect(() => {
    if (!session?.token) return
    setLoading(true)
    setNotFound(false)
    apiGetMeeting(session.token, meetingId)
      .then(setMeeting)
      .catch((error) => {
        if (error instanceof MeetingApiError && error.code === "not_found") {
          setNotFound(true)
        } else {
          toast.error(meetingErrorMessage(error))
        }
      })
      .finally(() => setLoading(false))
  }, [session?.token, meetingId])

  if (loading) {
    return (
      <div className="text-muted-foreground flex items-center gap-2 text-sm">
        <Loader2 className="size-4 animate-spin" />
        Loading meeting…
      </div>
    )
  }

  if (notFound || !meeting) {
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
              {meeting.attendees.length > 0 ? (
                <ul className="mt-2 space-y-1">
                  {meeting.attendees.map((a) => (
                    <li key={a} className="text-muted-foreground text-sm">{a}</li>
                  ))}
                </ul>
              ) : (
                <p className="text-muted-foreground mt-2 text-sm">No attendees listed.</p>
              )}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="space-y-4 p-6">
            <div>
              <p className="text-muted-foreground text-xs">Calendar</p>
              <p className="text-sm font-medium">{meeting.calendarLabel ?? "Unknown"}</p>
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
