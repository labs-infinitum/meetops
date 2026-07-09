import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { EmptyState } from "../components/EmptyState";
import { PageLoader } from "../components/Spinner";
import { api } from "../lib/api";
import type { Note } from "../types/api";

function formatDate(iso: string) {
  return new Date(iso).toLocaleString();
}

export function NotesPage() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    void api.get<Note[]>("/notes/").then((data) => {
      setNotes(data);
      setLoading(false);
    });
  }, []);

  if (loading) return <PageLoader />;

  const completed = notes.filter((n) => n.status === "completed");

  return (
    <div>
      <h1 className="font-display text-2xl font-bold">Notes</h1>
      <p className="mt-1 text-sm text-surface-500">AI-generated meeting summaries</p>

      {completed.length === 0 ? (
        <div className="mt-8">
          <EmptyState
            title="No notes yet"
            description="Notes appear after a recording is transcribed and summarized."
            action={
              <Link to="/app/recordings" className="btn-primary">
                View recordings
              </Link>
            }
          />
        </div>
      ) : (
        <div className="mt-8 space-y-3">
          {completed.map((n) => (
            <Link
              key={n.id}
              to={`/app/notes/${n.id}`}
              className="card block transition-shadow hover:shadow-md"
            >
              <div className="flex items-start justify-between gap-4">
                <div>
                  <h3 className="font-medium">{n.meeting_title ?? "Meeting notes"}</h3>
                  {n.tldr && <p className="mt-1 text-sm text-surface-600 dark:text-surface-400">{n.tldr}</p>}
                  <p className="mt-2 text-xs text-surface-500">{formatDate(n.created_at)}</p>
                </div>
                <span className="shrink-0 rounded-full bg-brand-50 px-2 py-0.5 text-xs font-medium text-brand-700 dark:bg-brand-950 dark:text-brand-300">
                  {n.status}
                </span>
              </div>
            </Link>
          ))}
        </div>
      )}

      {notes.length > completed.length && (
        <p className="mt-6 text-sm text-surface-500">
          {notes.length - completed.length} note(s) still processing…
        </p>
      )}
    </div>
  );
}
