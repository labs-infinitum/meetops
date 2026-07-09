import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Video } from "lucide-react";
import { EmptyState } from "@/components/EmptyState";
import { PageLoader } from "@/components/Spinner";
import { PageHeader } from "@/components/shared/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { api } from "@/lib/api";
import type { Meeting } from "@/types/api";

function formatDate(iso: string) {
  return new Date(iso).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function MeetingsPage() {
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    void api.get<Meeting[]>("/meetings/").then((data) => {
      setMeetings(data);
      setLoading(false);
    });
  }, []);

  if (loading) return <PageLoader />;

  const now = Date.now();
  const upcoming = meetings.filter((m) => new Date(m.end_at).getTime() >= now);
  const past = meetings.filter((m) => new Date(m.end_at).getTime() < now);

  return (
    <div className="space-y-8">
      <PageHeader
        title="Meetings"
        description="Upcoming and past meetings synced from your calendars."
        action={
          <Button asChild variant="outline">
            <Link to="/app/calendars">Connect calendar</Link>
          </Button>
        }
      />

      {meetings.length === 0 ? (
        <EmptyState
          title="No meetings yet"
          description="Connect Google Calendar and sync to see your scheduled Meet calls."
          action={
            <Button asChild>
              <Link to="/app/calendars">Connect calendar</Link>
            </Button>
          }
        />
      ) : (
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
  );
}

function MeetingRow({ meeting }: { meeting: Meeting }) {
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
            {meeting.auto_join && (
              <Badge variant="secondary" className="text-xs">
                Auto-record
              </Badge>
            )}
          </div>
          <p className="text-muted-foreground mt-1 text-sm">{formatDate(meeting.start_at)}</p>
          {meeting.meet_url && (
            <p className="text-muted-foreground mt-0.5 truncate text-xs">{meeting.meet_url}</p>
          )}
        </div>
        {meeting.meet_url && (
          <Button variant="outline" size="sm" asChild>
            <a href={meeting.meet_url} target="_blank" rel="noreferrer">
              <Video className="size-4" />
              Join
            </a>
          </Button>
        )}
      </CardContent>
    </Card>
  );
}
