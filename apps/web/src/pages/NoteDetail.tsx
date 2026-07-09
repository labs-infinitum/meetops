import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import ReactMarkdown from "react-markdown";
import { Alert } from "../components/Alert";
import { PageLoader } from "../components/Spinner";
import { ApiError, api } from "../lib/api";
import type { Note } from "../types/api";

function formatDate(iso: string) {
  return new Date(iso).toLocaleString();
}

export function NoteDetailPage() {
  const { id } = useParams<{ id: string }>();
  const [note, setNote] = useState<Note | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!id) return;
    void api
      .get<Note>(`/notes/${id}`)
      .then((n) => {
        setNote(n);
        setLoading(false);
      })
      .catch((err) => {
        if (err instanceof ApiError) setError(err.message);
        setLoading(false);
      });
  }, [id]);

  if (loading) return <PageLoader />;

  if (!note) {
    return (
      <div>
        <Alert>{error || "Note not found"}</Alert>
        <Link to="/app/notes" className="btn-ghost mt-4">
          ← Back
        </Link>
      </div>
    );
  }

  return (
    <div>
      <Link to="/app/notes" className="text-sm text-brand-600 hover:underline">
        ← Notes
      </Link>
      <h1 className="mt-4 font-display text-2xl font-bold">
        {note.meeting_title ?? "Meeting notes"}
      </h1>
      <p className="mt-1 text-sm text-surface-500">{formatDate(note.created_at)}</p>

      {note.tldr && (
        <div className="card mt-6 border-brand-200 bg-brand-50 dark:border-brand-900 dark:bg-brand-950">
          <p className="text-sm font-medium text-brand-800 dark:text-brand-200">TL;DR</p>
          <p className="mt-1">{note.tldr}</p>
        </div>
      )}

      {note.summary_md ? (
        <div className="card prose-meetops mt-6">
          <ReactMarkdown
            components={{
              h1: ({ children }) => (
                <h1 className="mb-4 text-2xl font-bold">{children}</h1>
              ),
              h2: ({ children }) => (
                <h2 className="mb-3 mt-6 text-xl font-semibold">{children}</h2>
              ),
              h3: ({ children }) => (
                <h3 className="mb-2 mt-4 text-lg font-semibold">{children}</h3>
              ),
              p: ({ children }) => <p className="mb-3 leading-relaxed">{children}</p>,
              ul: ({ children }) => <ul className="mb-3 list-disc pl-5">{children}</ul>,
              ol: ({ children }) => <ol className="mb-3 list-decimal pl-5">{children}</ol>,
              li: ({ children }) => <li className="mb-1">{children}</li>,
              a: ({ href, children }) => (
                <a href={href} className="text-brand-600 hover:underline" target="_blank" rel="noopener noreferrer">
                  {children}
                </a>
              ),
              code: ({ children }) => (
                <code className="rounded bg-surface-100 px-1 py-0.5 text-sm dark:bg-surface-800">
                  {children}
                </code>
              ),
            }}
          >
            {note.summary_md}
          </ReactMarkdown>
        </div>
      ) : (
        <div className="card mt-6">
          <p className="text-surface-500">
            {note.status === "completed"
              ? "No summary content available."
              : `Notes are ${note.status}. Check back soon.`}
          </p>
        </div>
      )}

      {note.action_items.length > 0 && (
        <div className="card mt-6">
          <h2 className="font-medium">Action items</h2>
          <ul className="mt-3 space-y-2">
            {note.action_items.map((item) => (
              <li key={item.id} className="flex items-start gap-2 text-sm">
                <span className={item.completed ? "text-surface-400 line-through" : ""}>
                  {item.text}
                </span>
                {item.owner && (
                  <span className="text-surface-500">({item.owner})</span>
                )}
              </li>
            ))}
          </ul>
          <Link to="/app/todos" className="mt-4 inline-block text-sm text-brand-600 hover:underline">
            Manage todos →
          </Link>
        </div>
      )}

      <Link
        to={`/app/recordings/${note.bot_run_id}`}
        className="mt-6 inline-block text-sm text-brand-600 hover:underline"
      >
        View recording →
      </Link>
    </div>
  );
}
