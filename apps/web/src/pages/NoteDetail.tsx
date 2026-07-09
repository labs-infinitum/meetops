import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import ReactMarkdown from "react-markdown";
import { ListChecks, Mic2, Sparkles } from "lucide-react";
import { Alert } from "@/components/Alert";
import { PageLoader } from "@/components/Spinner";
import { PageHeader } from "@/components/shared/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { ApiError, api } from "@/lib/api";
import type { Note } from "@/types/api";

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
      <div className="space-y-4">
        <PageHeader title="Note not found" />
        <Alert>{error || "Note not found"}</Alert>
        <Button asChild variant="outline">
          <Link to="/app/notes">Back to notes</Link>
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title={note.meeting_title ?? "Meeting notes"}
        description={formatDate(note.created_at)}
        action={
          <Button asChild variant="outline">
            <Link to={`/app/recordings/${note.bot_run_id}`}>View recording</Link>
          </Button>
        }
      />

      {note.tldr && (
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="flex items-center gap-2 text-base">
              <Sparkles className="text-primary size-4" />
              TL;DR
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm leading-relaxed">{note.tldr}</p>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Meeting notes</CardTitle>
        </CardHeader>
        <CardContent>
          {note.summary_md ? (
            <div className="prose prose-sm dark:prose-invert max-w-none">
              <ReactMarkdown
                components={{
                  h1: ({ children }) => (
                    <h1 className="mb-4 text-2xl font-bold">{children}</h1>
                  ),
                  h2: ({ children }) => (
                    <h2 className="mt-6 mb-3 text-xl font-semibold">{children}</h2>
                  ),
                  h3: ({ children }) => (
                    <h3 className="mt-4 mb-2 text-lg font-semibold">{children}</h3>
                  ),
                  p: ({ children }) => (
                    <p className="text-muted-foreground mb-3 text-sm leading-relaxed">
                      {children}
                    </p>
                  ),
                  ul: ({ children }) => (
                    <ul className="mb-3 list-disc pl-5">{children}</ul>
                  ),
                  ol: ({ children }) => (
                    <ol className="mb-3 list-decimal pl-5">{children}</ol>
                  ),
                  li: ({ children }) => (
                    <li className="text-muted-foreground mb-1 text-sm">{children}</li>
                  ),
                  a: ({ href, children }) => (
                    <a
                      href={href}
                      className="text-primary hover:underline"
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      {children}
                    </a>
                  ),
                  code: ({ children }) => (
                    <code className="bg-muted rounded px-1 py-0.5 text-sm">{children}</code>
                  ),
                }}
              >
                {note.summary_md}
              </ReactMarkdown>
            </div>
          ) : (
            <p className="text-muted-foreground text-sm">
              {note.status === "completed"
                ? "No summary content available."
                : `Notes are ${note.status}. Check back soon.`}
            </p>
          )}
        </CardContent>
      </Card>

      {note.action_items.length > 0 && (
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle className="flex items-center gap-2 text-base">
              <ListChecks className="size-4" />
              Action items
            </CardTitle>
            <Button variant="ghost" size="sm" asChild>
              <Link to="/app/todos">Manage todos</Link>
            </Button>
          </CardHeader>
          <CardContent className="space-y-2">
            {note.action_items.map((item) => (
              <div key={item.id} className="rounded-md border p-3">
                <p
                  className={`text-sm ${item.completed ? "text-muted-foreground line-through" : ""}`}
                >
                  {item.text}
                </p>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {item.owner && (
                    <Badge variant="outline" className="text-xs">
                      {item.owner}
                    </Badge>
                  )}
                  {item.due && (
                    <Badge variant="secondary" className="text-xs">
                      Due {item.due}
                    </Badge>
                  )}
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      )}

      {(note.cleaned_transcript || note.raw_transcript) && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Mic2 className="size-4" />
              Transcripts
            </CardTitle>
          </CardHeader>
          <CardContent className="grid gap-4 lg:grid-cols-2">
            <div>
              <h3 className="mb-2 text-sm font-semibold">Speaker-attributed</h3>
              {note.cleaned_transcript ? (
                <pre className="bg-muted text-muted-foreground max-h-96 overflow-auto rounded-lg border p-4 whitespace-pre-wrap text-sm leading-relaxed">
                  {note.cleaned_transcript}
                </pre>
              ) : (
                <p className="text-muted-foreground text-sm">Not available yet.</p>
              )}
            </div>
            <div>
              <h3 className="mb-2 text-sm font-semibold">Raw transcription</h3>
              {note.raw_transcript ? (
                <pre className="bg-muted text-muted-foreground max-h-96 overflow-auto rounded-lg border p-4 whitespace-pre-wrap text-xs leading-relaxed">
                  {note.raw_transcript}
                </pre>
              ) : (
                <p className="text-muted-foreground text-sm">Not available yet.</p>
              )}
            </div>
          </CardContent>
        </Card>
      )}

      <Separator />

      <Button variant="outline" asChild>
        <Link to="/app/notes">Back to notes</Link>
      </Button>
    </div>
  );
}
