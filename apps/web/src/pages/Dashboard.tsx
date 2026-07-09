import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import ReactMarkdown from "react-markdown";
import {
  CalendarDays,
  CheckSquare,
  ChevronRight,
  FileText,
  Video,
} from "lucide-react";
import { EmptyState } from "@/components/EmptyState";
import { PageLoader } from "@/components/Spinner";
import { PageHeader } from "@/components/shared/PageHeader";
import { StatCard } from "@/components/shared/StatCard";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { api } from "@/lib/api";
import type { Dashboard } from "@/types/api";

function formatDate(iso: string) {
  return new Date(iso).toLocaleString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function DashboardPage() {
  const [data, setData] = useState<Dashboard | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    void api.get<Dashboard>("/meetings/dashboard").then((d) => {
      setData(d);
      setLoading(false);
    });
  }, []);

  if (loading) return <PageLoader />;
  if (!data) return null;

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
        <StatCard
          title="Upcoming meetings"
          value={data.upcoming_count}
          icon={CalendarDays}
          subtitle="Synced from calendars"
        />
        <StatCard
          title="Recordings"
          value={data.recordings_count}
          icon={Video}
          subtitle="Bot runs"
        />
        <StatCard
          title="Notes"
          value={data.notes_count}
          icon={FileText}
          subtitle="AI summaries"
        />
        <StatCard
          title="Open todos"
          value={data.open_todos_count}
          icon={CheckSquare}
          subtitle="From meeting notes"
        />
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
            {data.upcoming.length === 0 ? (
              <EmptyState
                title="No upcoming meetings"
                description="Connect Google Calendar to sync your scheduled Meet calls."
                action={
                  <Button asChild>
                    <Link to="/app/calendars">Connect calendar</Link>
                  </Button>
                }
              />
            ) : (
              data.upcoming.map((m) => (
                <Link
                  key={m.id}
                  to={`/app/meetings/${m.id}`}
                  className="hover:bg-muted/50 flex items-center justify-between rounded-lg border p-3 transition-colors"
                >
                  <div className="min-w-0">
                    <p className="truncate text-sm font-medium">{m.title}</p>
                    <p className="text-muted-foreground text-xs">{formatDate(m.start_at)}</p>
                  </div>
                  {m.auto_join && (
                    <Badge variant="secondary" className="ml-3 shrink-0 text-xs">
                      Auto
                    </Badge>
                  )}
                </Link>
              ))
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base">Latest notes</CardTitle>
            <CardDescription>Most recent AI summary</CardDescription>
          </CardHeader>
          <CardContent>
            {data.latest_summary ? (
              <div className="space-y-3">
                <div>
                  <p className="text-sm font-medium">
                    {data.latest_summary.meeting_title ?? "Meeting"}
                  </p>
                  <p className="text-muted-foreground text-xs">
                    {formatDate(data.latest_summary.created_at)}
                  </p>
                </div>
                {data.latest_summary.tldr && (
                  <p className="text-sm">{data.latest_summary.tldr}</p>
                )}
                {data.latest_summary.summary_md && (
                  <div className="text-muted-foreground max-h-40 overflow-hidden text-sm">
                    <ReactMarkdown>{data.latest_summary.summary_md}</ReactMarkdown>
                  </div>
                )}
                <Button variant="ghost" size="sm" asChild className="px-0">
                  <Link to={`/app/notes/${data.latest_summary.id}`}>
                    Read full notes
                    <ChevronRight className="size-4" />
                  </Link>
                </Button>
              </div>
            ) : (
              <p className="text-muted-foreground text-sm">No notes yet.</p>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
