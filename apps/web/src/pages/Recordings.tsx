import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Alert } from "../components/Alert";
import { EmptyState } from "../components/EmptyState";
import { PageLoader, Spinner } from "../components/Spinner";
import { api } from "../lib/api";
import type { BotRun } from "../types/api";

const STATUS_COLORS: Record<string, string> = {
  completed: "bg-green-100 text-green-800 dark:bg-green-950 dark:text-green-300",
  failed: "bg-red-100 text-red-800 dark:bg-red-950 dark:text-red-300",
  cancelled: "bg-surface-100 text-surface-600 dark:bg-surface-800 dark:text-surface-400",
  recording: "bg-brand-100 text-brand-800 dark:bg-brand-950 dark:text-brand-300",
  queued: "bg-yellow-100 text-yellow-800 dark:bg-yellow-950 dark:text-yellow-300",
};

function formatDate(iso: string) {
  return new Date(iso).toLocaleString();
}

export function RecordingsPage() {
  const [recordings, setRecordings] = useState<BotRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [meetUrl, setMeetUrl] = useState("");
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    void api.get<BotRun[]>("/recordings/").then((data) => {
      setRecordings(data);
      setLoading(false);
    });
  }, []);

  async function startAdhoc(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    setStarting(true);
    try {
      const run = await api.post<BotRun>("/recordings/adhoc", { meet_url: meetUrl });
      setRecordings((prev) => [run, ...prev]);
      setMeetUrl("");
    } catch {
      setError("Failed to start recording. Check the Meet URL.");
    } finally {
      setStarting(false);
    }
  }

  if (loading) return <PageLoader />;

  return (
    <div>
      <h1 className="font-display text-2xl font-bold">Recordings</h1>
      <p className="mt-1 text-sm text-surface-500">Bot runs and meeting recordings</p>

      <form onSubmit={(e) => void startAdhoc(e)} className="card mt-6">
        <h2 className="font-medium">Record a Meet now</h2>
        <p className="mt-1 text-sm text-surface-500">
          Paste a Google Meet URL to start an ad-hoc recording
        </p>
        {error && (
          <div className="mt-3">
            <Alert>{error}</Alert>
          </div>
        )}
        <div className="mt-4 flex gap-2">
          <input
            className="input"
            placeholder="https://meet.google.com/abc-defg-hij"
            value={meetUrl}
            onChange={(e) => setMeetUrl(e.target.value)}
            required
          />
          <button type="submit" disabled={starting} className="btn-primary shrink-0">
            {starting ? <Spinner /> : "Start"}
          </button>
        </div>
      </form>

      {recordings.length === 0 ? (
        <div className="mt-8">
          <EmptyState
            title="No recordings yet"
            description="Start an ad-hoc recording or wait for a scheduled meeting to be recorded."
          />
        </div>
      ) : (
        <div className="mt-8 space-y-3">
          {recordings.map((r) => (
            <Link
              key={r.id}
              to={`/app/recordings/${r.id}`}
              className="card block transition-shadow hover:shadow-md"
            >
              <div className="flex items-start justify-between gap-4">
                <div>
                  <p className="font-medium">
                    {r.meet_url ?? `Recording #${r.id}`}
                  </p>
                  <p className="mt-1 text-sm text-surface-500">
                    {formatDate(r.created_at)} · {r.trigger}
                  </p>
                </div>
                <span
                  className={`shrink-0 rounded-full px-2 py-0.5 text-xs font-medium ${
                    STATUS_COLORS[r.status] ?? "bg-surface-100 text-surface-600"
                  }`}
                >
                  {r.status}
                </span>
              </div>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
