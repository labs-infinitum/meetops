import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Alert } from "../components/Alert";
import { PageLoader, Spinner } from "../components/Spinner";
import { ApiError, api } from "../lib/api";
import type { Meeting } from "../types/api";

function formatDate(iso: string) {
  return new Date(iso).toLocaleString();
}

export function MeetingDetailPage() {
  const { id } = useParams<{ id: string }>();
  const [meeting, setMeeting] = useState<Meeting | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!id) return;
    void api
      .get<Meeting>(`/meetings/${id}`)
      .then((m) => {
        setMeeting(m);
        setLoading(false);
      })
      .catch((err) => {
        if (err instanceof ApiError) setError(err.message);
        setLoading(false);
      });
  }, [id]);

  async function toggleAutoRecord() {
    if (!meeting) return;
    setSaving(true);
    try {
      const updated = await api.patch<Meeting>(`/meetings/${meeting.id}`, {
        auto_record: !meeting.auto_join,
      });
      setMeeting(updated);
    } catch (err) {
      if (err instanceof ApiError) setError(err.message);
    } finally {
      setSaving(false);
    }
  }

  if (loading) return <PageLoader />;

  if (!meeting) {
    return (
      <div>
        <Alert>{error || "Meeting not found"}</Alert>
        <Link to="/app/meetings" className="btn-ghost mt-4">
          ← Back to meetings
        </Link>
      </div>
    );
  }

  return (
    <div>
      <Link to="/app/meetings" className="text-sm text-brand-600 hover:underline">
        ← Meetings
      </Link>
      <h1 className="mt-4 font-display text-2xl font-bold">{meeting.title}</h1>

      {error && (
        <div className="mt-4">
          <Alert>{error}</Alert>
        </div>
      )}

      <div className="card mt-6 space-y-4">
        <div>
          <p className="text-sm text-surface-500">When</p>
          <p className="font-medium">
            {formatDate(meeting.start_at)} – {formatDate(meeting.end_at)}
          </p>
        </div>
        {meeting.description && (
          <div>
            <p className="text-sm text-surface-500">Description</p>
            <p className="whitespace-pre-wrap text-sm">{meeting.description}</p>
          </div>
        )}
        {meeting.meet_url && (
          <div>
            <p className="text-sm text-surface-500">Meet link</p>
            <a
              href={meeting.meet_url}
              target="_blank"
              rel="noopener noreferrer"
              className="text-brand-600 hover:underline"
            >
              {meeting.meet_url}
            </a>
          </div>
        )}
        <div className="flex items-center justify-between border-t border-surface-200 pt-4 dark:border-surface-700">
          <div>
            <p className="font-medium">Auto-record</p>
            <p className="text-sm text-surface-500">
              MeetOps bot joins and records this meeting automatically
            </p>
          </div>
          <button
            type="button"
            onClick={() => void toggleAutoRecord()}
            disabled={saving}
            className={`relative h-6 w-11 rounded-full transition-colors ${
              meeting.auto_join ? "bg-brand-600" : "bg-surface-300 dark:bg-surface-600"
            }`}
            role="switch"
            aria-checked={meeting.auto_join}
          >
            <span
              className={`absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white transition-transform ${
                meeting.auto_join ? "translate-x-5" : ""
              }`}
            />
            {saving && <Spinner className="absolute -right-8 h-4 w-4" />}
          </button>
        </div>
      </div>
    </div>
  );
}
