import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { EmptyState } from "../components/EmptyState";
import { PageLoader } from "../components/Spinner";
import { api } from "../lib/api";
import type { Meeting } from "../types/api";

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
    <div>
      <h1 className="font-display text-2xl font-bold">Meetings</h1>
      <p className="mt-1 text-sm text-surface-500">Synced from your Google Calendar</p>

      {meetings.length === 0 ? (
        <div className="mt-8">
          <EmptyState
            title="No meetings yet"
            description="Connect Google Calendar and sync to see your scheduled Meet calls."
            action={
              <Link to="/app/calendars" className="btn-primary">
                Connect calendar
              </Link>
            }
          />
        </div>
      ) : (
        <>
          {upcoming.length > 0 && (
            <section className="mt-8">
              <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-surface-500">
                Upcoming
              </h2>
              <div className="space-y-3">
                {upcoming.map((m) => (
                  <MeetingRow key={m.id} meeting={m} />
                ))}
              </div>
            </section>
          )}
          {past.length > 0 && (
            <section className="mt-8">
              <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-surface-500">
                Past
              </h2>
              <div className="space-y-3">
                {past.map((m) => (
                  <MeetingRow key={m.id} meeting={m} />
                ))}
              </div>
            </section>
          )}
        </>
      )}
    </div>
  );
}

function MeetingRow({ meeting }: { meeting: Meeting }) {
  return (
    <Link
      to={`/app/meetings/${meeting.id}`}
      className="card block transition-shadow hover:shadow-md"
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="font-medium">{meeting.title}</h3>
          <p className="mt-1 text-sm text-surface-500">{formatDate(meeting.start_at)}</p>
          {meeting.meet_url && (
            <p className="mt-1 truncate text-xs text-brand-600">{meeting.meet_url}</p>
          )}
        </div>
        {meeting.auto_join && (
          <span className="shrink-0 rounded-full bg-brand-50 px-2 py-0.5 text-xs font-medium text-brand-700 dark:bg-brand-950 dark:text-brand-300">
            Auto-record
          </span>
        )}
      </div>
    </Link>
  );
}
