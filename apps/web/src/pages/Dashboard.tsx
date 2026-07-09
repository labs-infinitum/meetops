import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import ReactMarkdown from "react-markdown";
import { EmptyState } from "../components/EmptyState";
import { PageLoader } from "../components/Spinner";
import { api } from "../lib/api";
import type { Dashboard } from "../types/api";

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

  const stats = [
    { label: "Upcoming meetings", value: data.upcoming_count, to: "/app/meetings" },
    { label: "Recordings", value: data.recordings_count, to: "/app/recordings" },
    { label: "Notes", value: data.notes_count, to: "/app/notes" },
    { label: "Open todos", value: data.open_todos_count, to: "/app/todos" },
  ];

  return (
    <div>
      <h1 className="font-display text-2xl font-bold">Dashboard</h1>
      <p className="mt-1 text-sm text-surface-500">Overview of your meetings and notes</p>

      <div className="mt-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {stats.map((s) => (
          <Link key={s.label} to={s.to} className="card transition-shadow hover:shadow-md">
            <p className="text-sm text-surface-500">{s.label}</p>
            <p className="mt-1 font-display text-3xl font-bold text-brand-600">{s.value}</p>
          </Link>
        ))}
      </div>

      <section className="mt-10">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="font-display text-lg font-semibold">Upcoming meetings</h2>
          <Link to="/app/meetings" className="text-sm text-brand-600 hover:underline">
            View all
          </Link>
        </div>
        {data.upcoming.length === 0 ? (
          <EmptyState
            title="No upcoming meetings"
            description="Connect Google Calendar to sync your scheduled Meet calls."
            action={
              <Link to="/app/calendars" className="btn-primary">
                Connect calendar
              </Link>
            }
          />
        ) : (
          <div className="space-y-3">
            {data.upcoming.map((m) => (
              <Link
                key={m.id}
                to={`/app/meetings/${m.id}`}
                className="card block transition-shadow hover:shadow-md"
              >
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <h3 className="font-medium">{m.title}</h3>
                    <p className="mt-1 text-sm text-surface-500">{formatDate(m.start_at)}</p>
                  </div>
                  {m.auto_join && (
                    <span className="rounded-full bg-brand-50 px-2 py-0.5 text-xs font-medium text-brand-700 dark:bg-brand-950 dark:text-brand-300">
                      Auto-record
                    </span>
                  )}
                </div>
              </Link>
            ))}
          </div>
        )}
      </section>

      {data.latest_summary && (
        <section className="mt-10">
          <h2 className="font-display text-lg font-semibold">Latest notes</h2>
          <div className="card mt-4">
            <p className="text-sm text-surface-500">
              {data.latest_summary.meeting_title ?? "Meeting"} ·{" "}
              {formatDate(data.latest_summary.created_at)}
            </p>
            {data.latest_summary.tldr && (
              <p className="mt-2 font-medium">{data.latest_summary.tldr}</p>
            )}
            {data.latest_summary.summary_md && (
              <div className="prose-meetops mt-4 max-h-48 overflow-hidden text-sm">
                <ReactMarkdown>{data.latest_summary.summary_md}</ReactMarkdown>
              </div>
            )}
            <Link
              to={`/app/notes/${data.latest_summary.id}`}
              className="mt-4 inline-block text-sm text-brand-600 hover:underline"
            >
              Read full notes →
            </Link>
          </div>
        </section>
      )}
    </div>
  );
}
