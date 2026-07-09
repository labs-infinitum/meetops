import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Alert } from "../components/Alert";
import { PageLoader, Spinner } from "../components/Spinner";
import { ApiError, api } from "../lib/api";
import type { BotRun, PlaybackUrl, TimelineEvent } from "../types/api";

function formatDate(iso: string) {
  return new Date(iso).toLocaleString();
}

export function RecordingDetailPage() {
  const { id } = useParams<{ id: string }>();
  const [run, setRun] = useState<BotRun | null>(null);
  const [playback, setPlayback] = useState<PlaybackUrl | null>(null);
  const [timeline, setTimeline] = useState<TimelineEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [stopping, setStopping] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!id) return;
    void (async () => {
      try {
        const [r, events] = await Promise.all([
          api.get<BotRun>(`/recordings/${id}`),
          api.get<TimelineEvent[]>(`/recordings/${id}/timeline`),
        ]);
        setRun(r);
        setTimeline(events);
        if (r.recording_path) {
          try {
            const pb = await api.get<PlaybackUrl>(`/recordings/${id}/playback-url`);
            setPlayback(pb);
          } catch {
            // playback may not be ready
          }
        }
      } catch (err) {
        if (err instanceof ApiError) setError(err.message);
      } finally {
        setLoading(false);
      }
    })();
  }, [id]);

  async function stopRecording() {
    if (!run) return;
    setStopping(true);
    try {
      const updated = await api.post<BotRun>(`/recordings/${run.id}/stop`);
      setRun(updated);
    } catch (err) {
      if (err instanceof ApiError) setError(err.message);
    } finally {
      setStopping(false);
    }
  }

  if (loading) return <PageLoader />;

  if (!run) {
    return (
      <div>
        <Alert>{error || "Recording not found"}</Alert>
        <Link to="/app/recordings" className="btn-ghost mt-4">
          ← Back
        </Link>
      </div>
    );
  }

  const isActive = !["completed", "failed", "cancelled"].includes(run.status);

  return (
    <div>
      <Link to="/app/recordings" className="text-sm text-brand-600 hover:underline">
        ← Recordings
      </Link>
      <div className="mt-4 flex items-start justify-between gap-4">
        <div>
          <h1 className="font-display text-2xl font-bold">
            {run.meet_url ?? `Recording #${run.id}`}
          </h1>
          <p className="mt-1 text-sm text-surface-500">
            Status: {run.status} · {run.trigger} · {formatDate(run.created_at)}
          </p>
        </div>
        {isActive && (
          <button
            type="button"
            onClick={() => void stopRecording()}
            disabled={stopping}
            className="btn-danger"
          >
            {stopping ? <Spinner /> : "Stop recording"}
          </button>
        )}
      </div>

      {error && (
        <div className="mt-4">
          <Alert>{error}</Alert>
        </div>
      )}

      {run.error && run.status !== "completed" && (
        <div className="mt-4">
          <Alert>{run.error}</Alert>
        </div>
      )}

      {playback && (
        <div className="card mt-6">
          <h2 className="font-medium">Playback</h2>
          <video
            controls
            playsInline
            className="mt-3 max-h-[70vh] w-full rounded-lg bg-black object-contain"
            src={playback.url}
          >
            Your browser does not support video playback.
          </video>
          <p className="mt-2 text-xs text-surface-500">
            URL expires {formatDate(playback.expires_at)}
          </p>
        </div>
      )}

      {timeline.length > 0 && (
        <div className="card mt-6">
          <h2 className="font-medium">Timeline</h2>
          <ul className="mt-4 max-h-96 space-y-2 overflow-y-auto text-sm">
            {timeline.map((ev) => (
              <li key={ev.id} className="flex gap-3 border-b border-surface-100 py-2 dark:border-surface-800">
                <span className="shrink-0 text-xs text-surface-500">
                  {formatDate(ev.occurred_at)}
                </span>
                <span className="font-medium">{ev.event_type}</span>
                {ev.participant_name && (
                  <span className="text-surface-500">{ev.participant_name}</span>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
