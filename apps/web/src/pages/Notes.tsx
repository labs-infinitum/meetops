import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { FileText, ListChecks } from "lucide-react";
import { EmptyState } from "@/components/EmptyState";
import { PageLoader } from "@/components/Spinner";
import { PageHeader } from "@/components/shared/PageHeader";
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
import type { Note } from "@/types/api";

function formatDate(iso: string) {
  return new Date(iso).toLocaleString();
}

function statusVariant(status: string): "default" | "secondary" | "destructive" | "outline" {
  switch (status) {
    case "completed":
      return "default";
    case "failed":
      return "destructive";
    case "processing":
      return "secondary";
    default:
      return "outline";
  }
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
  const processingCount = notes.length - completed.length;

  return (
    <div className="space-y-8">
      <PageHeader
        title="Notes"
        description="AI-generated summaries and transcripts from your meetings."
      />

      {completed.length === 0 ? (
        <EmptyState
          icon={FileText}
          title="No notes yet"
          description="Notes appear after a recording is transcribed and summarized."
          action={
            <Button asChild>
              <Link to="/app/recordings">View recordings</Link>
            </Button>
          }
        />
      ) : (
        <div className="grid gap-4">
          {completed.map((n) => (
            <Card key={n.id} className="transition-shadow hover:shadow-md">
              <CardHeader className="pb-3">
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <div>
                    <CardTitle className="text-base">
                      <Link to={`/app/notes/${n.id}`} className="hover:underline">
                        {n.meeting_title ?? "Meeting notes"}
                      </Link>
                    </CardTitle>
                    <CardDescription>{formatDate(n.created_at)}</CardDescription>
                  </div>
                  <Badge variant={statusVariant(n.status)} className="text-xs">
                    {n.status}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent>
                {n.tldr && (
                  <p className="text-muted-foreground text-sm leading-relaxed">{n.tldr}</p>
                )}
                {n.action_items.length > 0 && (
                  <p className="text-muted-foreground mt-3 flex items-center gap-1.5 text-xs">
                    <ListChecks className="size-3.5" />
                    {n.action_items.length} action item
                    {n.action_items.length !== 1 ? "s" : ""}
                  </p>
                )}
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      {processingCount > 0 && (
        <p className="text-muted-foreground text-sm">
          {processingCount} note(s) still processing…
        </p>
      )}
    </div>
  );
}
